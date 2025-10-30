// Suggested MIR Rust definitions for crates/aptos-fuzzer/src/mir/ir.rs
// Designed to be a compact, expressive IR for Move interactions (MIR)
// Focus: enums, structs, and traits representing types, values, resource locations,
// facts (preconditions), effects (postconditions), calls, and chains.
// Derive common traits for easy hashing/serialization and use in fuzzing pipelines.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// High-level type tags (mirrors Aptos' TypeTag but simplified)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeTagLite {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<TypeTagLite>),
    Struct(StructTag),
    TypeVar(u32), // a type parameter placeholder
}

impl Serialize for TypeTagLite {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TypeTagLite::Bool => serializer.serialize_str("Bool"),
            TypeTagLite::U8 => serializer.serialize_str("U8"),
            TypeTagLite::U64 => serializer.serialize_str("U64"),
            TypeTagLite::U128 => serializer.serialize_str("U128"),
            TypeTagLite::Address => serializer.serialize_str("Address"),
            TypeTagLite::Signer => serializer.serialize_str("Signer"),
            TypeTagLite::Vector(inner) => {
                use serde::ser::SerializeStruct;
                let mut s = serializer.serialize_struct("Vector", 1)?;
                s.serialize_field("Vector", inner.as_ref())?;
                s.end()
            }
            TypeTagLite::Struct(s) => s.serialize(serializer),
            TypeTagLite::TypeVar(v) => {
                use serde::ser::SerializeStruct;
                let mut st = serializer.serialize_struct("TypeVar", 1)?;
                st.serialize_field("TypeVar", v)?;
                st.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for TypeTagLite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct TypeTagVisitor;

        impl<'de> Visitor<'de> for TypeTagVisitor {
            type Value = TypeTagLite;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a TypeTagLite string or struct")
            }

            fn visit_str<E>(self, value: &str) -> Result<TypeTagLite, E>
            where
                E: de::Error,
            {
                match value {
                    "Bool" => Ok(TypeTagLite::Bool),
                    "U8" => Ok(TypeTagLite::U8),
                    "U64" => Ok(TypeTagLite::U64),
                    "U128" => Ok(TypeTagLite::U128),
                    "Address" => Ok(TypeTagLite::Address),
                    "Signer" => Ok(TypeTagLite::Signer),
                    _ => Err(E::custom(format!("Unknown type: {}", value))),
                }
            }

            fn visit_map<M>(self, mut map: M) -> Result<TypeTagLite, M::Error>
            where
                M: MapAccess<'de>,
            {
                if let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "Vector" => {
                            let inner: TypeTagLite = map.next_value()?;
                            Ok(TypeTagLite::Vector(Box::new(inner)))
                        }
                        "Struct" => {
                            let s: StructTag = map.next_value()?;
                            Ok(TypeTagLite::Struct(s))
                        }
                        "TypeVar" => {
                            let v: u32 = map.next_value()?;
                            Ok(TypeTagLite::TypeVar(v))
                        }
                        _ => Err(de::Error::unknown_field(&key, &["Vector", "Struct", "TypeVar"])),
                    }
                } else {
                    Err(de::Error::custom("expected a type tag"))
                }
            }
        }

        deserializer.deserialize_any(TypeTagVisitor)
    }
}

/// A struct tag: (address, module, name, optional type args)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StructTag {
    pub address: String, // use string for textual module address (e.g., 0x1)
    pub module: String,
    pub name: String,
    pub ty_args: Vec<TypeTagLite>,
}

/// A variable id used inside MIR programs. Variables carry a type tag when known.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Var {
    pub id: u32,
    pub ty: Option<TypeTagLite>,
}

impl Var {
    pub fn new(id: u32) -> Self {
        Self { id, ty: None }
    }
    pub fn with_ty(id: u32, ty: TypeTagLite) -> Self {
        Self { id, ty: Some(ty) }
    }
}

/// Address expressions — how an address is computed or referenced. This is
/// intentionally lightweight but expressive enough for object addresses.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AddrExpr {
    /// A concrete literal address, e.g. 0x1
    Literal(String),
    /// An address coming from a Var (e.g., signer var)
    Var(Var),
    /// Object address computed as object::create_object_address(owner, seed_expr)
    ObjectAddr {
        owner: Box<AddrExpr>,
        seed: SeedExpr,
    },
    /// Derived from a call result (call id + result var)
    FromCall { call_id: usize, result: Var },
    /// Address read from a resource field at runtime (e.g., seller from Listing)
    /// This allows fuzzer to track data dependencies across resources
    FromField {
        res: Box<ResLoc>,
        field_path: String,
    },
    /// Named object address computed deterministically from creator and seed
    /// e.g., object::create_named_object(creator, seed)
    NamedObjectAddr {
        creator: Box<AddrExpr>,
        seed: SeedExpr,
    },
}

/// Seed expressions for object addresses (small domain expressions)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeedExpr {
    /// A small integer literal
    Lit(u64),
    /// A small variable (often a u64 local)
    Var(Var),
    /// A named counter (e.g., user_todo_counter)
    Counter(String),
    /// Bytes literal seed
    Bytes(Vec<u8>),
    /// String converted to bytes seed
    Str(String),
    /// Composite seed combining multiple components (e.g., issuer_addr || holder_addr)
    /// Useful for paired object addresses like (issuer, holder) -> holding_address
    Composite(Vec<SeedExpr>),
    /// Seed from a function call result
    FromCall {
        module: String,
        function: String,
        args: Vec<SeedExpr>,
        /// Optional expansion of the function's internal implementation
        /// e.g., for construct_todo_list_object_seed, this would contain the
        /// BcsToBytes(Format { ... }) expression
        expansion: Option<Box<SeedExpr>>,
    },
    /// BCS serialization of an expression
    BcsToBytes(Box<SeedExpr>),
    /// String formatting operation (e.g., format!("{}_{}", addr, counter))
    Format {
        template: String,
        args: Vec<SeedExpr>,
    },
    /// Address literal (for use in seed generation)
    Address(String),
}

/// Resource locator — identifies a resource type and the address where it lives
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResLoc {
    pub struct_name: String,  // Reference to struct in struct_defs, e.g. "@TodoList"
    pub addr: AddrExpr,
}

/// Facts: preconditions the fuzzer can reason about (Exists, NotExists, bounds, field equality...)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fact {
    Exists(ResLoc),
    NotExists(ResLoc),
    /// A weak fact: length at field_path is >= n. field_path is a simple dot-separated string.
    LengthAtLeast {
        res: ResLoc,
        field_path: String,
        n: usize,
    },
    /// Field equals a literal value
    FieldEq {
        res: ResLoc,
        field_path: String,
        value: Literal,
    },
    /// Field exists (for optional fields)
    FieldExists {
        res: ResLoc,
        field_path: String,
    },
    /// Field does not exist (for optional fields)
    FieldNotExists {
        res: ResLoc,
        field_path: String,
    },
    /// Vector/SmartVector contains an address value
    VectorContains {
        res: ResLoc,
        field_path: String,
        value: AddrExpr,
    },
    /// Vector/SmartVector does not contain an address value
    VectorNotContains {
        res: ResLoc,
        field_path: String,
        value: AddrExpr,
    },
    /// Numeric field comparison: field >= value
    FieldGte {
        res: ResLoc,
        field_path: String,
        value: u64,
    },
    /// Numeric field comparison: field > value
    FieldGt {
        res: ResLoc,
        field_path: String,
        value: u64,
    },
    /// Boolean AND of two facts (for complex conditions)
    And(Box<Fact>, Box<Fact>),
    /// Boolean OR of two facts (for alternative conditions)
    Or(Box<Fact>, Box<Fact>),
}

/// Simple literal values that can appear in args or facts
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Literal {
    Bool(bool),
    U64(u64),
    Address(String),
    Bytes(Vec<u8>),
    Str(String),
}

/// Effects — best-effort predictions of what a call produces
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    Creates(ResLoc),
    Deletes(ResLoc),
    Writes(ResLoc),
    EmitsEvent {
        tag: String,
        fields: Vec<Literal>,
    },
    /// e.g., mints coin amounts, or transfers
    Mints {
        coin_ty: TypeTagLite,
        amount: u128,
        to: AddrExpr,
    },
    /// returns a new object address (for convenience)
    NewObjectAddr {
        result: Var,
        addr: AddrExpr,
    },
}

/// The kind of argument passed to a call. Can be a literal, a var reference, or a ResLoc.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Arg {
    Lit(Literal),
    Var(Var),
    /// a reference to a resource location (resolved to an address expression)
    Res(ResLoc),
}

/// A MIR Call — describing a Move entry function or script call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    /// Unique id within a Chain (helps attribute coverage)
    pub id: usize,
    /// canonical module address (string) and module name
    pub module_addr: String,
    pub module: String,
    pub function: String,

    /// Type args (concrete or TypeVar placeholders)
    pub ty_args: Vec<TypeTagLite>,

    /// Arguments (in order). These are either Vars (which themselves may be bound to AddrExpr) or literals.
    pub args: Vec<Arg>,

    /// Resources listed in `acquires` in the function signature (struct name references)
    pub acquires: Vec<String>,

    /// Static + inferred requires
    pub requires: Vec<Fact>,

    /// Predicted effects for provider search and chaining
    pub effects: Vec<Effect>,

    /// Return type of the function (if any)
    pub ret: Option<TypeTagLite>,
}

impl Call {
    pub fn new(
        id: usize,
        module_addr: impl Into<String>,
        module: impl Into<String>,
        function: impl Into<String>,
    ) -> Self {
        Self {
            id,
            module_addr: module_addr.into(),
            module: module.into(),
            function: function.into(),
            ty_args: vec![],
            args: vec![],
            acquires: vec![],
            requires: vec![],
            effects: vec![],
            ret: None,
        }
    }

    pub fn description(&self) -> String {
        format!(
            "{}::{}::{}(ty_args={:?}, args={:?})",
            self.module_addr, self.module, self.function, self.ty_args, self.args
        )
    }
}

/// Field definition for a struct
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeTagLite,
}

/// Complete struct definition including fields
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub tag: StructTag,
    pub fields: Vec<FieldDef>,
}

/// A Chain (program) — sequence of Calls with an optional final return var.
/// Chains are the unit the fuzzer will mutate, insert provider calls into, and execute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub calls: Vec<Call>,
    /// Map of struct name to struct definition (including nested structs)
    pub struct_defs: BTreeMap<String, StructDef>,
    /// a small, optional metadata bag to store learned state snapshots etc.
    pub metadata: BTreeMap<String, String>,
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

impl Chain {
    pub fn new() -> Self {
        Self {
            calls: vec![],
            struct_defs: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }
    pub fn push(&mut self, c: Call) {
        self.calls.push(c);
    }
    pub fn len(&self) -> usize {
        self.calls.len()
    }
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
    
    pub fn sort_by_dependencies(&mut self) {
        use std::collections::{HashMap, HashSet};
        
        let mut creates: HashMap<usize, HashSet<String>> = HashMap::new();
        let mut requires: HashMap<usize, HashSet<String>> = HashMap::new();
        
        for (idx, call) in self.calls.iter().enumerate() {
            for effect in &call.effects {
                if let Effect::Creates(res_loc) = effect {
                    creates.entry(idx).or_default().insert(res_loc.struct_name.clone());
                }
            }
            
            for fact in &call.requires {
                if let Fact::Exists(res_loc) = fact {
                    requires.entry(idx).or_default().insert(res_loc.struct_name.clone());
                }
            }
        }
        
        let mut sorted_indices = Vec::new();
        let mut satisfied_structs: HashSet<String> = HashSet::new();
        let mut remaining: HashSet<usize> = (0..self.calls.len()).collect();
        
        while !remaining.is_empty() {
            let mut made_progress = false;
            
            let candidates: Vec<usize> = remaining.iter()
                .filter(|&&idx| {
                    requires.get(&idx)
                        .map(|reqs| reqs.iter().all(|s| satisfied_structs.contains(s)))
                        .unwrap_or(true)
                })
                .copied()
                .collect();
            
            for idx in candidates {
                sorted_indices.push(idx);
                remaining.remove(&idx);
                
                if let Some(created) = creates.get(&idx) {
                    satisfied_structs.extend(created.iter().cloned());
                }
                
                made_progress = true;
            }
            
            if !made_progress {
                let mut rest: Vec<_> = remaining.iter().copied().collect();
                rest.sort();
                sorted_indices.extend(rest);
                break;
            }
        }
        
        let mut new_calls = Vec::with_capacity(self.calls.len());
        for idx in sorted_indices {
            new_calls.push(self.calls[idx].clone());
        }
        self.calls = new_calls;
    }
    
    pub fn calls_creating_struct(&self, struct_name: &str) -> Vec<usize> {
        self.calls.iter().enumerate()
            .filter(|(_, call)| {
                call.effects.iter().any(|eff| {
                    matches!(eff, Effect::Creates(res) if res.struct_name == struct_name)
                })
            })
            .map(|(idx, _)| idx)
            .collect()
    }
    
    pub fn calls_requiring_struct(&self, struct_name: &str) -> Vec<usize> {
        self.calls.iter().enumerate()
            .filter(|(_, call)| {
                call.requires.iter().any(|fact| {
                    matches!(fact, Fact::Exists(res) if res.struct_name == struct_name)
                })
            })
            .map(|(idx, _)| idx)
            .collect()
    }
}

// --------------------------- Traits & Helpers -------------------------------

/// A trait to unify address expressions with concrete addresses or other expressions.
/// Implementations can do simple structural unification or heuristic binding.
pub trait UnifyAddr {
    /// Try to unify `self` with `other` and return a (possibly partial) substitution
    fn unify(&self, other: &AddrExpr) -> Option<BTreeMap<String, AddrExpr>>;
}

impl UnifyAddr for AddrExpr {
    fn unify(&self, other: &AddrExpr) -> Option<BTreeMap<String, AddrExpr>> {
        // Substitution map: keys are identifiers like "v{id}" for Var bindings
        // and "seed:{name}" for named seed bindings. Values are AddrExprs.
        type Subst = BTreeMap<String, AddrExpr>;
        fn unify_seed(s1: &SeedExpr, s2: &SeedExpr, subst: &mut Subst) -> bool {
            match (s1, s2) {
                (SeedExpr::Lit(a), SeedExpr::Lit(b)) => a == b,
                (SeedExpr::Var(v), other) => {
                    let k = format!("v{}", v.id);
                    subst.insert(k, AddrExpr::from_seed(other.clone()));
                    true
                }
                (SeedExpr::Counter(n), SeedExpr::Counter(m)) => n == m,
                (SeedExpr::Lit(_a), SeedExpr::Var(v)) => {
                    let k = format!("v{}", v.id);
                    subst.insert(k, AddrExpr::from_seed(s1.clone()));
                    true
                }
                // conservative fallback: don't unify different kinds
                _ => false,
            }
        }

        let mut map: Subst = BTreeMap::new();
        // Helper to bind a var key -> addrexpr, but keep existing binding consistent
        let bind = |k: String, v: AddrExpr, map: &mut Subst| -> bool {
            if let Some(existing) = map.get(&k) {
                existing == &v
            } else {
                map.insert(k, v);
                true
            }
        };

        match (self, other) {
            // identical literals
            (AddrExpr::Literal(a), AddrExpr::Literal(b)) if a == b => Some(map),

            // a Var can unify with anything: record substitution v{id} -> other
            (AddrExpr::Var(v), other) => {
                let k = format!("v{}", v.id);
                if bind(k, other.clone(), &mut map) {
                    Some(map)
                } else {
                    None
                }
            }
            (other, AddrExpr::Var(v)) => {
                let k = format!("v{}", v.id);
                if bind(k, other.clone(), &mut map) {
                    Some(map)
                } else {
                    None
                }
            }

            // ObjectAddr <> ObjectAddr: unify owner and seed
            (
                AddrExpr::ObjectAddr {
                    owner: o1,
                    seed: s1,
                },
                AddrExpr::ObjectAddr {
                    owner: o2,
                    seed: s2,
                },
            ) => {
                if let Some(m1) = o1.unify(o2) {
                    // merge maps
                    for (k, v) in m1.into_iter() {
                        map.insert(k, v);
                    }
                    // unify seeds (allow binding seed vars)
                    if unify_seed(s1, s2, &mut map) {
                        Some(map)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            // FromCall reverse binding:
            // If one side is FromCall{call_id, result} and the other is a concrete AddrExpr,
            // bind a special key `call:{call_id}:r{varid}` -> addrexpr so callers can substitute
            (AddrExpr::FromCall { call_id, result }, other) => {
                let k = format!("call:{}:r{}", call_id, result.id);
                if bind(k, other.clone(), &mut map) {
                    Some(map)
                } else {
                    None
                }
            }
            (other, AddrExpr::FromCall { call_id, result }) => {
                let k = format!("call:{}:r{}", call_id, result.id);
                if bind(k, other.clone(), &mut map) {
                    Some(map)
                } else {
                    None
                }
            }

            // Fallback: try structural equality for simple cases
            (a, b) if a == b => Some(map),

            _ => None,
        }
    }
}

// Utilities to convert a SeedExpr into a trivial AddrExpr when recording seed bindings
impl AddrExpr {
    fn from_seed(s: SeedExpr) -> AddrExpr {
        match s {
            SeedExpr::Lit(n) => AddrExpr::Literal(format!("seed:{}", n)),
            SeedExpr::Var(v) => AddrExpr::Var(v),
            SeedExpr::Counter(name) => AddrExpr::Literal(format!("counter:{}", name)),
            SeedExpr::Bytes(b) => AddrExpr::Literal(format!("bytes:{}", hex::encode(b))),
            SeedExpr::Str(s) => AddrExpr::Literal(format!("str:{}", s)),
            SeedExpr::Composite(_) => AddrExpr::Literal("composite_seed".to_string()),
            SeedExpr::FromCall { .. } => AddrExpr::Literal("from_call_seed".to_string()),
            SeedExpr::BcsToBytes(_) => AddrExpr::Literal("bcs_seed".to_string()),
            SeedExpr::Format { .. } => AddrExpr::Literal("format_seed".to_string()),
            SeedExpr::Address(a) => AddrExpr::Literal(a),
        }
    }
}

/// Helpers to pretty-print Facts/Effects for logging during fuzzing
impl std::fmt::Display for Fact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fact::Exists(r) => write!(f, "Exists({} @ {:?})", r.struct_name, r.addr),
            Fact::NotExists(r) => write!(f, "NotExists({} @ {:?})", r.struct_name, r.addr),
            Fact::LengthAtLeast { res, field_path, n } => {
                write!(f, "Len({}.{}) >= {}", res.struct_name, field_path, n)
            }
            Fact::FieldEq {
                res,
                field_path,
                value,
            } => write!(
                f,
                "FieldEq({}.{} == {:?})",
                res.struct_name, field_path, value
            ),
            Fact::FieldExists { res, field_path } => {
                write!(f, "FieldExists({}.{})", res.struct_name, field_path)
            }
            Fact::FieldNotExists { res, field_path } => {
                write!(f, "FieldNotExists({}.{})", res.struct_name, field_path)
            }
            Fact::VectorContains {
                res,
                field_path,
                value,
            } => write!(
                f,
                "VectorContains({}.{}, {:?})",
                res.struct_name, field_path, value
            ),
            Fact::VectorNotContains {
                res,
                field_path,
                value,
            } => write!(
                f,
                "VectorNotContains({}.{}, {:?})",
                res.struct_name, field_path, value
            ),
            Fact::FieldGte {
                res,
                field_path,
                value,
            } => write!(
                f,
                "FieldGte({}.{} >= {})",
                res.struct_name, field_path, value
            ),
            Fact::FieldGt {
                res,
                field_path,
                value,
            } => write!(
                f,
                "FieldGt({}.{} > {})",
                res.struct_name, field_path, value
            ),
            Fact::And(a, b) => write!(f, "({} AND {})", a, b),
            Fact::Or(a, b) => write!(f, "({} OR {})", a, b),
        }
    }
}

// --------------------------- Example utility constructors ------------------

/// Shorthand to build a ResLoc for tests / generator code
pub fn resloc_simple(struct_name: &str, addr: AddrExpr) -> ResLoc {
    ResLoc {
        struct_name: format!("@{}", struct_name),
        addr,
    }
}

// -------------------- Chain to Aptos Transaction Conversion --------------------

use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::ModuleId;
use aptos_types::transaction::EntryFunction;
use aptos_vm::aptos_vm::FUZZER_SENDER;
use bcs;

impl Chain {
    /// Convert Chain calls to Input calls
    pub fn to_entry_functions(&self) -> Result<Vec<EntryFunction>, String> {
        let mut functions = Vec::new();
        
        for call in &self.calls {
            let entry_fn = call_to_entry_function(call)?;
            functions.push(entry_fn);
        }
        
        Ok(functions)
    }
}

fn call_to_entry_function(call: &Call) -> Result<EntryFunction, String> {
    // Parse module address
    let addr_bytes = hex::decode(&call.module_addr)
        .map_err(|e| format!("Invalid module address: {}", e))?;
    if addr_bytes.len() != 32 {
        return Err(format!("Module address must be 32 bytes, got {}", addr_bytes.len()));
    }
    let mut addr_array = [0u8; 32];
    addr_array.copy_from_slice(&addr_bytes);
    let module_addr = AccountAddress::new(addr_array);
    
    // Parse module and function names
    let module_name = Identifier::new(call.module.as_str())
        .map_err(|e| format!("Invalid module name: {}", e))?;
    let function_name = Identifier::new(call.function.as_str())
        .map_err(|e| format!("Invalid function name: {}", e))?;
    
    let module_id = ModuleId::new(module_addr, module_name);
    
    let ty_args = vec![];
    
    // Convert args to BCS-encoded values (skip Signer type as it's provided by sender)
    let mut bcs_args = Vec::new();
    for arg in &call.args {
        match arg {
            Arg::Var(var) => {
                if let Some(ref ty) = var.ty {
                    // Skip Signer type - it's automatically provided by the transaction sender
                    if matches!(ty, TypeTagLite::Signer) {
                        continue;
                    }
                    let bytes = generate_value_for_type(ty)?;
                    bcs_args.push(bytes);
                } else {
                    return Err(format!("Var {} has no type information", var.id));
                }
            }
            Arg::Lit(lit) => {
                let bytes = literal_to_bcs(lit)?;
                bcs_args.push(bytes);
            }
            Arg::Res(_res) => {
                return Err("Resource arguments not yet supported".to_string());
            }
        }
    }
    
    let entry_fn = EntryFunction::new(module_id, function_name, ty_args, bcs_args);
    Ok(entry_fn)
}

fn generate_value_for_type(ty: &TypeTagLite) -> Result<Vec<u8>, String> {
    match ty {
        TypeTagLite::Bool => bcs::to_bytes(&false).map_err(|e| e.to_string()),
        TypeTagLite::U8 => bcs::to_bytes(&0u8).map_err(|e| e.to_string()),
        TypeTagLite::U64 => bcs::to_bytes(&0u64).map_err(|e| e.to_string()),
        TypeTagLite::U128 => bcs::to_bytes(&0u128).map_err(|e| e.to_string()),
        TypeTagLite::Address => bcs::to_bytes(&FUZZER_SENDER).map_err(|e| e.to_string()),
        TypeTagLite::Signer => bcs::to_bytes(&FUZZER_SENDER).map_err(|e| e.to_string()),
        TypeTagLite::Vector(_inner) => {
            let empty: Vec<u8> = vec![];
            bcs::to_bytes(&empty).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unsupported type for value generation: {:?}", ty)),
    }
}

fn literal_to_bcs(lit: &Literal) -> Result<Vec<u8>, String> {
    match lit {
        Literal::Bool(b) => bcs::to_bytes(b).map_err(|e| e.to_string()),
        Literal::U64(n) => bcs::to_bytes(n).map_err(|e| e.to_string()),
        Literal::Address(s) => {
            let addr_bytes = hex::decode(s)
                .map_err(|e| format!("Invalid address: {}", e))?;
            if addr_bytes.len() != 32 {
                return Err(format!("Address must be 32 bytes"));
            }
            let mut addr_array = [0u8; 32];
            addr_array.copy_from_slice(&addr_bytes);
            let addr = AccountAddress::new(addr_array);
            bcs::to_bytes(&addr).map_err(|e| e.to_string())
        }
        Literal::Bytes(b) => bcs::to_bytes(b).map_err(|e| e.to_string()),
        Literal::Str(s) => bcs::to_bytes(s).map_err(|e| e.to_string()),
    }
}