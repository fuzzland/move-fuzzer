//! Individual mutation strategy implementations
//!
//! Each strategy focuses on a specific type of mutation algorithm that can be
//! composed together to target different types of vulnerabilities or edge
//! cases.

pub mod boundary_value;
pub mod power_of_two;
pub mod random;

pub use boundary_value::*;
pub use power_of_two::*;
pub use random::*;
