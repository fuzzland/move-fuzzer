use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use fuzzer_core::{ChainAdapter, FunctionInfo, FuzzerConfig, ObjectChange, Parameter, ViolationInfo};
use sui_json_rpc_types::{SuiMoveNormalizedFunction, SuiMoveNormalizedModule, SuiMoveNormalizedType};
use sui_move_core_types::language_storage::TypeTag;
use sui_move_core_types::u256::U256;
use sui_sdk::{SuiClient, SuiClientBuilder};
use sui_simulator::Simulator;
use sui_tracer::shift_violation_tracer::ShiftViolationTracer;
use sui_types::base_types::{ObjectID, SequenceNumber, SuiAddress};
use sui_types::object::Object;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::{Argument, InputObjectKind, ObjectArg, ObjectReadResultKind, TransactionData};
use sui_types::type_input::TypeInput;
use sui_types::Identifier;
use tracing::{debug, info};

pub mod error;
pub mod mutation;
pub mod types;

pub use error::*;
pub use mutation::orchestrator::SuiMutationOrchestrator;
pub use types::*;

/// Macro to extract homogeneous vector elements
macro_rules! extract_vector {
    ($vec:expr, $variant:ident, $type:ty) => {
        $vec.iter()
            .map(|v| match v {
                CloneableValue::$variant(val) => Ok(*val),
                _ => bail!("Mixed types in vector"),
            })
            .collect::<Result<Vec<$type>>>()
    };
}

/// Sui implementation of the ChainAdapter trait
pub struct SuiAdapter {
    client: Arc<SuiClient>,
    simulator: sui_simulator::DBSimulator,
}

impl SuiAdapter {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        info!("🔧 Creating SuiAdapter with RPC URL: {}", rpc_url);

        let client = Arc::new(SuiClientBuilder::default().build(rpc_url).await?);

        info!("🔧 Initializing Sui simulator with database access");
        let simulator = sui_simulator::DBSimulator::new(rpc_url).await?;

        info!("✅ SuiAdapter initialized successfully");
        Ok(Self { client, simulator })
    }

    /// Helper method to add pure arguments with unified error handling
    fn add_pure_arg<T>(ptb: &mut ProgrammableTransactionBuilder, value: T) -> Result<Argument>
    where
        T: serde::Serialize,
    {
        ptb.pure(value).with_context(|| "Failed to add pure argument")
    }

    /// Handle vector argument building
    fn build_vector_argument(ptb: &mut ProgrammableTransactionBuilder, vec: &[CloneableValue]) -> Result<Argument> {
        if vec.is_empty() {
            return Self::add_pure_arg(ptb, Vec::<u8>::new());
        }

        match &vec[0] {
            CloneableValue::U8(_) => Self::add_pure_arg(ptb, extract_vector!(vec, U8, u8)?),
            CloneableValue::U16(_) => Self::add_pure_arg(ptb, extract_vector!(vec, U16, u16)?),
            CloneableValue::U32(_) => Self::add_pure_arg(ptb, extract_vector!(vec, U32, u32)?),
            CloneableValue::U64(_) => Self::add_pure_arg(ptb, extract_vector!(vec, U64, u64)?),
            CloneableValue::U128(_) => Self::add_pure_arg(ptb, extract_vector!(vec, U128, u128)?),
            CloneableValue::U256(_) => {
                let primitives = vec
                    .iter()
                    .map(|v| match v {
                        CloneableValue::U256(bytes) => Ok(U256::from_be_bytes(bytes)),
                        _ => bail!("Mixed types in u256 vector"),
                    })
                    .collect::<Result<Vec<U256>>>()?;
                Self::add_pure_arg(ptb, primitives)
            }
            CloneableValue::Bool(_) => Self::add_pure_arg(ptb, extract_vector!(vec, Bool, bool)?),
            CloneableValue::Address(_) => Self::add_pure_arg(ptb, extract_vector!(vec, Address, SuiAddress)?),
            _ => bail!("Unsupported vector element type: {:?}", vec[0]),
        }
    }

    /// Build transaction arguments from CloneableValue
    fn build_transaction_argument(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        value: &CloneableValue,
    ) -> Result<Argument> {
        match value {
            // Basic types - use unified error handling
            CloneableValue::U8(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::U16(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::U32(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::U64(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::U128(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::U256(bytes) => Self::add_pure_arg(ptb, *bytes),
            CloneableValue::Bool(v) => Self::add_pure_arg(ptb, *v),
            CloneableValue::Address(addr) => Self::add_pure_arg(ptb, *addr),

            // Vector - delegate to specialized method
            CloneableValue::Vector(vec) => Self::build_vector_argument(ptb, vec),

            // UID - create object reference
            CloneableValue::UID { id } => {
                let obj_ref = (
                    *id,
                    SequenceNumber::from_u64(1),
                    sui_types::digests::ObjectDigest::OBJECT_DIGEST_WRAPPED,
                );
                ptb.obj(ObjectArg::ImmOrOwnedObject(obj_ref))
                    .with_context(|| "Failed to add UID argument")
            }

            // StructObject - handle ownership and caching
            CloneableValue::StructObject { ownership_type, .. } => {
                let sui_object = value.get_struct_object()?;

                let obj_ref = sui_object.compute_object_reference();

                let obj_arg = match ownership_type {
                    ObjectOwnershipType::Owned => ObjectArg::ImmOrOwnedObject(obj_ref),
                    ObjectOwnershipType::MutableShared { initial_shared_version } => ObjectArg::SharedObject {
                        id: obj_ref.0,
                        initial_shared_version: *initial_shared_version,
                        mutable: true,
                    },
                    ObjectOwnershipType::ImmutableShared => ObjectArg::SharedObject {
                        id: obj_ref.0,
                        initial_shared_version: SequenceNumber::from_u64(1),
                        mutable: false,
                    },
                };

                ptb.obj(obj_arg).with_context(|| "Failed to add object argument")
            }

        }
    }

    async fn fetch_package_modules(&self, package_id: &ObjectID) -> Result<BTreeMap<String, SuiMoveNormalizedModule>> {
        let package = self
            .client
            .read_api()
            .get_normalized_move_modules_by_package(*package_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch package modules: {}", e))?;
        Ok(package)
    }

    fn find_function<'a>(
        &self,
        modules: &'a BTreeMap<String, SuiMoveNormalizedModule>,
        module_name: &str,
        function_name: &str,
    ) -> Result<&'a SuiMoveNormalizedFunction> {
        let module = modules
            .get(module_name)
            .ok_or_else(|| anyhow::anyhow!("Module '{}' not found", module_name))?;

        let function = module
            .exposed_functions
            .get(function_name)
            .ok_or_else(|| anyhow::anyhow!("Function '{}' not found in module '{}'", function_name, module_name))?;

        Ok(function)
    }
}

#[async_trait]
impl ChainAdapter for SuiAdapter {
    type Value = CloneableValue;
    type Address = SuiAddress;
    type ObjectId = ObjectID;
    type Object = Object;
    type ExecutionResult = ExecutionResult;
    type Mutator = SuiMutationOrchestrator;

    async fn resolve_function(&self, config: &FuzzerConfig) -> Result<FunctionInfo> {
        info!(
            "Resolving function: {}::{}::{}",
            config.package_id, config.module_name, config.function_name
        );

        Ok(FunctionInfo {
            package_id: config.package_id.clone(),
            module_name: config.module_name.clone(),
            function_name: config.function_name.clone(),
            type_arguments: config.type_arguments.clone(),
        })
    }

    async fn initialize_parameters(
        &self,
        function: &FunctionInfo,
        args: &[String],
    ) -> Result<Vec<Parameter<Self::Value>>> {
        info!(
            "Initializing parameters for function: {}::{}",
            function.module_name, function.function_name
        );

        let package_id = ObjectID::from_hex_literal(&function.package_id)?;
        let modules = self.fetch_package_modules(&package_id).await?;
        let sui_function = self.find_function(&modules, &function.module_name, &function.function_name)?;

        // Parse type arguments to TypeInput for parameter resolution
        let type_inputs: Vec<TypeInput> = Self::parse_type_arguments(&function.type_arguments)?
            .into_iter()
            .map(|tag| TypeInput::from(tag))
            .collect();

        let mut parameters = Vec::new();

        for (index, (param_type, arg)) in sui_function.parameters.iter().zip(args.iter()).enumerate() {
            let param_name = format!("param_{}", index);
            let value = self.parse_parameter_value(arg, param_type, &type_inputs).await?;

            parameters.push(Parameter {
                index,
                name: param_name,
                type_name: format!("{:?}", param_type),
                value,
            });
        }

        info!("Initialized {} parameters", parameters.len());
        Ok(parameters)
    }

    async fn execute(
        &self,
        sender: &Self::Address,
        function: &FunctionInfo,
        params: &[Parameter<Self::Value>],
    ) -> Result<Self::ExecutionResult> {
        let start_time = Instant::now();
        info!(
            "🚀 Executing function {}::{}::{} with {} parameters, sender: {}",
            function.package_id,
            function.module_name,
            function.function_name,
            params.len(),
            sender
        );

        // Log parameter details for debugging
        for (i, param) in params.iter().enumerate() {
            debug!("  Parameter {}: {} = {:?}", i, param.name, param.value);
        }

        let package_id = ObjectID::from_hex_literal(&function.package_id)?;
        let module_identifier = Identifier::from_str(&function.module_name)?;
        let function_identifier = Identifier::from_str(&function.function_name)?;

        // Build programmable transaction
        let mut ptb = ProgrammableTransactionBuilder::new();
        let mut tx_args = Vec::new();
        let mut struct_objects = Vec::new();

        for param in params.iter() {
            // Collect StructObject parameters for override_objects
            if matches!(&param.value, CloneableValue::StructObject { .. }) {
                let sui_object = param.value.get_struct_object_owned()?;
                debug!(
                    "Using {} object for parameter {}: {}",
                    if param.value.has_cached_object() {
                        "cached"
                    } else {
                        "initial"
                    },
                    param.name,
                    sui_object.id()
                );
                struct_objects.push((sui_object.id(), sui_object));
            }

            tx_args.push(self.build_transaction_argument(&mut ptb, &param.value)?);
        }

        debug!(
            "Adding function call to transaction: {}::{}",
            module_identifier, function_identifier
        );
        ptb.programmable_move_call(
            package_id,
            module_identifier,
            function_identifier,
            Self::parse_type_arguments(&function.type_arguments)?,
            tx_args,
        );

        let pt = ptb.finish();

        // Create gas coin for the transaction
        let gas_balance = 1_000_000_000_000u64;
        debug!("Creating gas coin with balance {} for sender {}", gas_balance, sender);
        let gas_coin = Object::new_gas_with_balance_and_owner_for_testing(gas_balance, *sender);
        let gas_payment = vec![gas_coin.compute_object_reference()];

        // Combine gas coin with struct objects for override_objects
        let mut override_objects = vec![(gas_coin.id(), gas_coin)];
        override_objects.extend(struct_objects);

        let gas_budget = 10_000_000_000u64;
        let gas_price = 1_000u64;
        let tx_data = TransactionData::new_programmable(*sender, gas_payment, pt, gas_budget, gas_price);

        // Create tracer for shift violation detection
        debug!("Creating shift violation tracer");
        let tracer = ShiftViolationTracer::new();
        let shift_violations_handle = tracer.shift_violations();

        // Execute simulation with tracer
        info!(
            "🔄 Simulating transaction with {} override objects ({} gas + {} struct objects)",
            override_objects.len(),
            1,
            override_objects.len() - 1
        );
        let simulate_result = self
            .simulator
            .simulate(tx_data, override_objects, Some(Box::new(tracer)))
            .await?;

        let execution_time = start_time.elapsed();

        let shift_violations = shift_violations_handle
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to acquire shift violations lock: {}", e))?
            .clone();

        info!(
            ?simulate_result,
            ?shift_violations,
            ?execution_time,
            "✅ Execution completed"
        );

        Ok(ExecutionResult {
            simulate_result,
            shift_violations,
            execution_time,
        })
    }

    fn has_shift_violations(&self, result: &Self::ExecutionResult) -> bool {
        !result.shift_violations.is_empty()
    }

    fn extract_violations(&self, result: &Self::ExecutionResult) -> Vec<ViolationInfo> {
        result
            .shift_violations
            .iter()
            .map(|violation| {
                let location_str = format!(
                    "{}::{}:{}",
                    violation.location.module, violation.location.function, violation.location.pc
                );

                let parsed_value = violation.value.parse::<u64>().unwrap_or_default();

                ViolationInfo {
                    location: location_str,
                    operation: violation.instruction.clone(),
                    left_operand: parsed_value,
                    right_operand: violation.shift_amount as u64,
                }
            })
            .collect()
    }

    fn extract_object_changes(
        &self,
        result: &Self::ExecutionResult,
    ) -> Vec<ObjectChange<Self::ObjectId, Self::Object>> {
        let mut changes = Vec::new();

        for change in &result.simulate_result.object_changes {
            if let InputObjectKind::SharedMoveObject { id, mutable: true, .. } = &change.input_object_kind {
                if let ObjectReadResultKind::Object(obj) = &change.object {
                    changes.push(ObjectChange {
                        id: *id,
                        object: obj.clone(),
                    });
                }
            }
        }

        changes
    }

    fn get_sender_from_config(&self, config: &FuzzerConfig) -> Self::Address {
        if let Some(sender_str) = &config.sender {
            SuiAddress::from_str(sender_str).unwrap_or_default()
        } else {
            SuiAddress::ZERO
        }
    }

    fn compute_object_digest(&self, object: &Self::Object) -> Vec<u8> {
        object.digest().into_inner().to_vec()
    }

    fn update_value_with_cached_object(&self, value: &mut Self::Value, object: &Self::Object) -> Result<()> {
        if let CloneableValue::StructObject { cached_object, .. } = value {
            *cached_object = Some(object.clone());
            debug!("Updated cached object for struct object: {}", object.id());
        } else {
            debug!("Cannot update non-struct object value with cached object");
        }
        Ok(())
    }

    fn bytes_to_object_id(&self, bytes: &[u8]) -> Result<Self::ObjectId> {
        ObjectID::from_bytes(bytes).map_err(|e| anyhow::anyhow!("Failed to convert bytes to ObjectID: {}", e))
    }

    fn object_id_to_bytes(&self, id: &Self::ObjectId) -> Vec<u8> {
        id.to_vec()
    }

    fn create_mutator(&self) -> Self::Mutator {
        SuiMutationOrchestrator::new()
    }
}

impl SuiAdapter {
    async fn parse_parameter_value(
        &self,
        arg: &str,
        param_type: &SuiMoveNormalizedType,
        type_arguments: &[TypeInput],
    ) -> Result<CloneableValue> {
        // First unwrap reference types to get the actual type to process
        let unwrapped_type = crate::types::unwrap_reference_type(param_type);

        match unwrapped_type {
            SuiMoveNormalizedType::U8 => Ok(CloneableValue::U8(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::U16 => Ok(CloneableValue::U16(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::U32 => Ok(CloneableValue::U32(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::U64 => Ok(CloneableValue::U64(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::U128 => Ok(CloneableValue::U128(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::U256 => Ok(CloneableValue::parse_u256(arg)?),
            SuiMoveNormalizedType::Bool => Ok(CloneableValue::Bool(arg.parse().unwrap_or_default())),
            SuiMoveNormalizedType::Address => Ok(CloneableValue::Address(
                SuiAddress::from_str(arg).unwrap_or_else(|_| SuiAddress::random_for_testing_only()),
            )),
            SuiMoveNormalizedType::Vector(inner_type) => Ok(CloneableValue::parse_vector(inner_type, arg)?),
            // Handle struct types by fetching object from blockchain
            SuiMoveNormalizedType::Struct { .. } => {
                Ok(CloneableValue::from_object_id(arg, &self.client, param_type).await?)
            }
            // Handle type parameters - resolve to concrete type and recurse
            SuiMoveNormalizedType::TypeParameter(index) => {
                let resolved_type = crate::types::resolve_type_parameter(*index as usize, type_arguments)?;
                Box::pin(self.parse_parameter_value(arg, &resolved_type, type_arguments)).await
            }
            param_type => {
                bail!("Unsupported parameter type: {:?}", param_type)
            }
        }
    }

    fn parse_type_arguments(type_args: &[String]) -> Result<Vec<TypeTag>> {
        type_args
            .iter()
            .map(|s| TypeTag::from_str(s).with_context(|| format!("Invalid type argument '{}': failed to parse", s)))
            .collect()
    }
}
