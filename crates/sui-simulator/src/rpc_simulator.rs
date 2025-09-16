use async_trait::async_trait;
use sui_json_rpc_types::SuiObjectDataOptions;
use sui_move_trace_format::interface::Tracer;
use sui_sdk::rpc_types::SuiProtocolConfigValue;
use sui_sdk::{SuiClient, SuiClientBuilder};
use sui_types::base_types::ObjectID;
use sui_types::object::Object;
use sui_types::transaction::TransactionData;

use super::{SimulateResult, Simulator};
use crate::SimulatorError;

#[derive(Clone)]
pub struct RpcSimulator {
    pub client: SuiClient,
}

impl RpcSimulator {
    pub async fn new(url: impl AsRef<str>) -> Self {
        let client = SuiClientBuilder::default()
            .max_concurrent_requests(2000)
            .build(url)
            .await
            .unwrap();

        Self { client }
    }

    pub async fn max_budget(&self) -> u64 {
        let cfg = self
            .client
            .read_api()
            .get_protocol_config(None)
            .await
            .expect("failed to get config");

        let Some(Some(SuiProtocolConfigValue::U64(max))) = cfg.attributes.get("max_tx_gas") else {
            panic!("failed to get max_tx_gas");
        };

        *max
    }
}

#[async_trait]
impl Simulator for RpcSimulator {
    async fn simulate(
        &self,
        tx_data: TransactionData,
        override_objects: Vec<(ObjectID, Object)>,
        _tracer: Option<Box<dyn Tracer + Send>>,
    ) -> Result<SimulateResult, SimulatorError> {
        let resp = self
            .client
            .read_api()
            .dry_run_transaction_block_override(tx_data, override_objects)
            .await
            .map_err(|e| SimulatorError::ExecutionError(e.to_string()))?;

        Ok(SimulateResult {
            effects: resp.effects,
            events: resp.events,
            object_changes: vec![],
            balance_changes: resp.balance_changes,
        })
    }

    fn name(&self) -> &str {
        "RpcSimulator"
    }

    async fn get_object(&self, obj_id: &ObjectID) -> Option<Object> {
        self.client
            .read_api()
            .get_object_with_options(*obj_id, SuiObjectDataOptions::bcs_lossless())
            .await
            .ok()?
            .data?
            .try_into()
            .ok()
    }

    async fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        let mut objects = Vec::new();
        for id in object_ids {
            objects.push(self.get_object(id).await);
        }
        objects
    }
}
