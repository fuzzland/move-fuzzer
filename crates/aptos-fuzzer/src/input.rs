use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Deserialize, Serialize)]
pub struct AptosFuzzerInput;

impl Input for AptosFuzzerInput {}
