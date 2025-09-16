use std::sync::{Arc, Mutex};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sui_move_binary_format::file_format::Bytecode;
use sui_move_core_types::language_storage::ModuleId;
use sui_move_core_types::u256::U256;
use sui_move_trace_format::format::{Effect, TraceEvent, TraceValue};
use sui_move_trace_format::interface::{Tracer, Writer};
use sui_move_trace_format::value::SerializableMoveValue;
use sui_move_vm_types::values::IntegerValue;
use tracing::warn;

use crate::whitelist::WhitelistChecker;

/// Maximum allowed frame stack depth to prevent stack overflow
const MAX_FRAME_DEPTH: usize = 1000;

/// A custom Move tracer that monitors shl violations
#[derive(Debug)]
pub struct ShiftViolationTracer {
    // Shift violations for shared access
    shift_violations: Arc<Mutex<Vec<ShiftViolation>>>,
    whitelist_checker: Arc<WhitelistChecker>,
    // Frame stack for tracking nested function calls
    frame_stack: Vec<FrameInfo>,
    // Current instruction information
    current_instruction: Option<InstructionInfo>,
    // Buffer for operands (value, shift_amount)
    operand_buffer: Vec<IntegerValue>,
}

#[derive(Debug, Clone)]
struct FrameInfo {
    module: ModuleId,
    function: String,
}

#[derive(Debug, Clone)]
struct InstructionInfo {
    bytecode: Bytecode,
    pc: u16,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ShiftViolation {
    pub instruction: String,
    pub value: String,
    pub shift_amount: u8,
    pub location: InstructionLocation,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, JsonSchema, Hash)]
#[serde(rename_all = "camelCase")]
pub struct InstructionLocation {
    pub module: String,
    pub function: String,
    pub pc: u16,
}

impl ShiftViolationTracer {
    pub fn new() -> Self {
        let shift_violations = Arc::new(Mutex::new(Vec::new()));
        Self {
            shift_violations,
            whitelist_checker: Arc::new(WhitelistChecker::default()),
            frame_stack: Vec::new(),
            current_instruction: None,
            operand_buffer: Vec::new(),
        }
    }

    pub fn shift_violations(&self) -> Arc<Mutex<Vec<ShiftViolation>>> {
        self.shift_violations.clone()
    }

    pub fn check_truncation(value: &IntegerValue, shift_amount: u8) -> bool {
        let check_leading_zeros = |leading_zeros: u32| shift_amount > leading_zeros as u8;

        match value {
            IntegerValue::U8(v) => check_leading_zeros(v.leading_zeros()),
            IntegerValue::U16(v) => check_leading_zeros(v.leading_zeros()),
            IntegerValue::U32(v) => check_leading_zeros(v.leading_zeros()),
            IntegerValue::U64(v) => check_leading_zeros(v.leading_zeros()),
            IntegerValue::U128(v) => check_leading_zeros(v.leading_zeros()),
            IntegerValue::U256(v) => check_leading_zeros(v.leading_zeros()),
        }
    }

    fn extract_integer_value(trace_value: &TraceValue) -> Option<IntegerValue> {
        match trace_value {
            TraceValue::RuntimeValue { value } => match value {
                SerializableMoveValue::U8(v) => Some(IntegerValue::U8(*v)),
                SerializableMoveValue::U16(v) => Some(IntegerValue::U16(*v)),
                SerializableMoveValue::U32(v) => Some(IntegerValue::U32(*v)),
                SerializableMoveValue::U64(v) => Some(IntegerValue::U64(*v)),
                SerializableMoveValue::U128(v) => Some(IntegerValue::U128(*v)),
                SerializableMoveValue::U256(v) => Some(IntegerValue::U256(*v)),
                _ => None,
            },
            _ => None,
        }
    }

    fn handle_shl_instruction(&mut self) {
        if self.operand_buffer.len() < 2 {
            return;
        }

        let value = self.operand_buffer.pop().unwrap();
        let shift_amount = self.operand_buffer.pop().unwrap();

        let shift_amount = match shift_amount {
            IntegerValue::U8(v) => v,
            IntegerValue::U16(v) => v as u8,
            IntegerValue::U32(v) => v as u8,
            IntegerValue::U64(v) => v as u8,
            IntegerValue::U128(v) => v as u8,
            IntegerValue::U256(v) => {
                if v <= U256::from(255u8) {
                    v.to_string().parse::<u8>().unwrap_or(u8::MAX)
                } else {
                    u8::MAX
                }
            }
        };

        if !Self::check_truncation(&value, shift_amount) {
            return;
        }

        if let Some(frame) = self.frame_stack.last() {
            if let Some(instr) = &self.current_instruction {
                let location = InstructionLocation {
                    module: frame.module.to_string(),
                    function: frame.function.clone(),
                    pc: instr.pc,
                };

                if self
                    .whitelist_checker
                    .should_ignore(&location.module, &location.function)
                {
                    return;
                }

                let violation = ShiftViolation {
                    instruction: format!("{:?}", instr.bytecode),
                    value: format!("{:?}", value),
                    shift_amount,
                    location,
                };
                warn!("Shift violation detected: {:?}", violation);
                if let Ok(mut violations) = self.shift_violations.lock() {
                    if !violations.contains(&violation) {
                        violations.push(violation);
                    }
                }
            }
        }

        self.operand_buffer.clear();
    }
}

impl Tracer for ShiftViolationTracer {
    fn notify(&mut self, event: &TraceEvent, _writer: Writer<'_>) {
        match event {
            TraceEvent::OpenFrame { frame, .. } => {
                if self.frame_stack.len() >= MAX_FRAME_DEPTH {
                    tracing::warn!(
                        "Frame stack depth exceeded limit ({}), ignoring frame: {}::{}",
                        MAX_FRAME_DEPTH,
                        frame.module,
                        frame.function_name
                    );
                    return;
                }

                self.frame_stack.push(FrameInfo {
                    module: frame.module.clone(),
                    function: frame.function_name.clone(),
                });
            }
            TraceEvent::CloseFrame { .. } => {
                if self.frame_stack.pop().is_none() {
                    tracing::warn!("Attempted to close frame but stack is empty");
                }

                if self.frame_stack.is_empty() {
                    self.current_instruction = None;
                    self.operand_buffer.clear();
                }
            }
            TraceEvent::Instruction { pc, instruction, .. } => {
                if self.frame_stack.is_empty() {
                    return;
                }

                if instruction.contains("SHL") {
                    self.current_instruction = Some(InstructionInfo {
                        bytecode: Bytecode::Shl,
                        pc: *pc,
                    });
                    self.operand_buffer.clear();
                }
            }
            TraceEvent::Effect(effect) => {
                if self.frame_stack.is_empty() {
                    return;
                }

                if let Some(instr) = &self.current_instruction {
                    if instr.bytecode == Bytecode::Shl {
                        match effect.as_ref() {
                            Effect::Pop(trace_value) => {
                                if let Some(int_val) = Self::extract_integer_value(trace_value) {
                                    self.operand_buffer.push(int_val);

                                    if self.operand_buffer.len() == 2 {
                                        self.handle_shl_instruction();
                                        self.current_instruction = None;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_leading_zeros_edge_cases() {
        assert_eq!(255u8.leading_zeros(), 0);
        assert_eq!(65535u16.leading_zeros(), 0);
        assert_eq!(u32::MAX.leading_zeros(), 0);
        assert_eq!(u64::MAX.leading_zeros(), 0);
        assert_eq!(u128::MAX.leading_zeros(), 0);

        assert_eq!(1u8.leading_zeros(), 7);
        assert_eq!(1u16.leading_zeros(), 15);
        assert_eq!(1u32.leading_zeros(), 31);
        assert_eq!(1u64.leading_zeros(), 63);
        assert_eq!(1u128.leading_zeros(), 127);

        assert_eq!(0u8.leading_zeros(), 8);
        assert_eq!(0u16.leading_zeros(), 16);
        assert_eq!(0u32.leading_zeros(), 32);
        assert_eq!(0u64.leading_zeros(), 64);
        assert_eq!(0u128.leading_zeros(), 128);
    }

    #[test]
    fn test_check_truncation_u8() {
        let value_u8_max = IntegerValue::U8(255);
        let value_u8_small = IntegerValue::U8(15);
        let value_u8_zero = IntegerValue::U8(0);

        assert!(ShiftViolationTracer::check_truncation(&value_u8_max, 1));
        assert!(ShiftViolationTracer::check_truncation(&value_u8_small, 5));
        assert!(ShiftViolationTracer::check_truncation(&value_u8_zero, 9));

        assert!(!ShiftViolationTracer::check_truncation(&value_u8_small, 4));
        assert!(!ShiftViolationTracer::check_truncation(&value_u8_small, 3));
        assert!(!ShiftViolationTracer::check_truncation(&value_u8_zero, 8));
    }

    #[test]
    fn test_check_truncation_u16() {
        let value_u16_max = IntegerValue::U16(65535);
        let value_u16_small = IntegerValue::U16(15);

        assert!(ShiftViolationTracer::check_truncation(&value_u16_max, 1));
        assert!(!ShiftViolationTracer::check_truncation(&value_u16_small, 12));
        assert!(ShiftViolationTracer::check_truncation(&value_u16_small, 13));
    }

    #[test]
    fn test_check_truncation_u32() {
        let value_u32_max = IntegerValue::U32(u32::MAX);
        let value_u32_small = IntegerValue::U32(255);

        assert!(ShiftViolationTracer::check_truncation(&value_u32_max, 1));
        assert!(!ShiftViolationTracer::check_truncation(&value_u32_small, 24));
        assert!(ShiftViolationTracer::check_truncation(&value_u32_small, 25));
    }

    #[test]
    fn test_check_truncation_u64() {
        let value_u64_large = IntegerValue::U64(0xFFFFFFFFFFFFFFFF);
        let value_u64_small = IntegerValue::U64(0x0F);

        assert!(ShiftViolationTracer::check_truncation(&value_u64_large, 1));
        assert!(!ShiftViolationTracer::check_truncation(&value_u64_small, 59));
        assert!(ShiftViolationTracer::check_truncation(&value_u64_small, 61));
    }

    #[test]
    fn test_check_truncation_u128() {
        let value_u128_max = IntegerValue::U128(u128::MAX);
        let value_u128_small = IntegerValue::U128(255);

        assert!(ShiftViolationTracer::check_truncation(&value_u128_max, 1));
        assert!(!ShiftViolationTracer::check_truncation(&value_u128_small, 120));
        assert!(ShiftViolationTracer::check_truncation(&value_u128_small, 121));
    }

    #[test]
    fn test_check_truncation_u256() {
        let value_u256_max = IntegerValue::U256(U256::max_value());
        let value_u256_small = IntegerValue::U256(U256::from(255u32));

        assert!(ShiftViolationTracer::check_truncation(&value_u256_max, 1));
        assert!(!ShiftViolationTracer::check_truncation(&value_u256_small, 247));
        assert!(ShiftViolationTracer::check_truncation(&value_u256_small, 249));
    }

    #[test]
    fn test_extract_integer_value() {
        let trace_value_u8 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U8(42),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u8);
        assert!(result.is_some());
        if let Some(IntegerValue::U8(val)) = result {
            assert_eq!(val, 42);
        } else {
            panic!("Expected U8 value");
        }

        let trace_value_u16 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U16(1000),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u16);
        assert!(result.is_some());
        if let Some(IntegerValue::U16(val)) = result {
            assert_eq!(val, 1000);
        } else {
            panic!("Expected U16 value");
        }

        let trace_value_u32 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U32(100000),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u32);
        assert!(result.is_some());
        if let Some(IntegerValue::U32(val)) = result {
            assert_eq!(val, 100000);
        } else {
            panic!("Expected U32 value");
        }

        let trace_value_u64 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U64(1000000000),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u64);
        assert!(result.is_some());
        if let Some(IntegerValue::U64(val)) = result {
            assert_eq!(val, 1000000000);
        } else {
            panic!("Expected U64 value");
        }

        let trace_value_u128 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U128(1000000000000u128),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u128);
        assert!(result.is_some());
        if let Some(IntegerValue::U128(val)) = result {
            assert_eq!(val, 1000000000000u128);
        } else {
            panic!("Expected U128 value");
        }

        let trace_value_u256 = TraceValue::RuntimeValue {
            value: SerializableMoveValue::U256(U256::from(1000u32)),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_u256);
        assert!(result.is_some());
        if let Some(IntegerValue::U256(val)) = result {
            assert_eq!(val, U256::from(1000u32));
        } else {
            panic!("Expected U256 value");
        }

        let trace_value_bool = TraceValue::RuntimeValue {
            value: SerializableMoveValue::Bool(true),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_bool);
        assert!(result.is_none());

        let trace_value_vector = TraceValue::RuntimeValue {
            value: SerializableMoveValue::Vector(vec![]),
        };
        let result = ShiftViolationTracer::extract_integer_value(&trace_value_vector);
        assert!(result.is_none());
    }
}
