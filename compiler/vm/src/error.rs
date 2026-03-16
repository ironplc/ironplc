use core::fmt;

/// Runtime traps that halt VM execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    DivideByZero,
    StackOverflow,
    StackUnderflow,
    InvalidInstruction(u8),
    InvalidConstantIndex(u16),
    InvalidVariableIndex(u16),
    InvalidFunctionId(u16),
    WatchdogTimeout(u16),
    NegativeExponent,
    InvalidBuiltinFunction(u16),
    DataRegionOutOfBounds(u32),
    TempBufferExhausted,
    InvalidFbTypeId(u16),
    ArrayIndexOutOfBounds {
        var_index: u16,
        index: i32,
        total_elements: u32,
    },
}

// v_code() and exit_code() are generated from resources/problem-codes.csv
include!(concat!(env!("OUT_DIR"), "/trap_codes.rs"));

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trap::DivideByZero => write!(f, "divide by zero"),
            Trap::StackOverflow => write!(f, "stack overflow"),
            Trap::StackUnderflow => write!(f, "stack underflow"),
            Trap::InvalidInstruction(op) => write!(f, "invalid instruction: 0x{op:02X}"),
            Trap::InvalidConstantIndex(i) => write!(f, "invalid constant index: {i}"),
            Trap::InvalidVariableIndex(i) => write!(f, "invalid variable index: {i}"),
            Trap::InvalidFunctionId(id) => write!(f, "invalid function ID: {id}"),
            Trap::WatchdogTimeout(id) => write!(f, "watchdog timeout on task {id}"),
            Trap::NegativeExponent => write!(f, "negative exponent"),
            Trap::InvalidBuiltinFunction(id) => {
                write!(f, "invalid built-in function: 0x{id:04X}")
            }
            Trap::DataRegionOutOfBounds(offset) => {
                write!(f, "data region access out of bounds at offset {offset}")
            }
            Trap::TempBufferExhausted => write!(f, "temporary buffer pool exhausted"),
            Trap::InvalidFbTypeId(id) => {
                write!(f, "invalid FB type ID: 0x{id:04X}")
            }
            Trap::ArrayIndexOutOfBounds {
                var_index,
                index,
                total_elements,
            } => {
                write!(
                    f,
                    "array index out of bounds: index {index} for array variable {var_index} with {total_elements} elements"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_display_when_watchdog_timeout_then_includes_task_id() {
        let trap = Trap::WatchdogTimeout(3);
        assert_eq!(format!("{trap}"), "watchdog timeout on task 3");
    }

    #[test]
    fn v_code_when_divide_by_zero_then_v4001() {
        assert_eq!(Trap::DivideByZero.v_code(), "V4001");
    }

    #[test]
    fn v_code_when_negative_exponent_then_v4002() {
        assert_eq!(Trap::NegativeExponent.v_code(), "V4002");
    }

    #[test]
    fn v_code_when_watchdog_timeout_then_v4003() {
        assert_eq!(Trap::WatchdogTimeout(0).v_code(), "V4003");
    }

    #[test]
    fn v_code_when_stack_overflow_then_v9001() {
        assert_eq!(Trap::StackOverflow.v_code(), "V9001");
    }

    #[test]
    fn v_code_when_stack_underflow_then_v9002() {
        assert_eq!(Trap::StackUnderflow.v_code(), "V9002");
    }

    #[test]
    fn v_code_when_invalid_instruction_then_v9003() {
        assert_eq!(Trap::InvalidInstruction(0xFF).v_code(), "V9003");
    }

    #[test]
    fn v_code_when_invalid_constant_index_then_v9004() {
        assert_eq!(Trap::InvalidConstantIndex(42).v_code(), "V9004");
    }

    #[test]
    fn v_code_when_invalid_variable_index_then_v9005() {
        assert_eq!(Trap::InvalidVariableIndex(7).v_code(), "V9005");
    }

    #[test]
    fn v_code_when_invalid_function_id_then_v9006() {
        assert_eq!(Trap::InvalidFunctionId(3).v_code(), "V9006");
    }

    #[test]
    fn v_code_when_invalid_builtin_function_then_v9007() {
        assert_eq!(Trap::InvalidBuiltinFunction(0x0101).v_code(), "V9007");
    }

    #[test]
    fn v_code_when_array_index_out_of_bounds_then_v4005() {
        assert_eq!(
            Trap::ArrayIndexOutOfBounds {
                var_index: 0,
                index: 10,
                total_elements: 5,
            }
            .v_code(),
            "V4005"
        );
    }

    #[test]
    fn v_code_when_invalid_fb_type_id_then_v9010() {
        assert_eq!(Trap::InvalidFbTypeId(0x0010).v_code(), "V9010");
    }

    #[test]
    fn exit_code_when_user_error_then_1() {
        assert_eq!(Trap::DivideByZero.exit_code(), 1);
        assert_eq!(Trap::NegativeExponent.exit_code(), 1);
        assert_eq!(Trap::WatchdogTimeout(0).exit_code(), 1);
        assert_eq!(
            Trap::ArrayIndexOutOfBounds {
                var_index: 0,
                index: 0,
                total_elements: 0,
            }
            .exit_code(),
            1
        );
    }

    #[test]
    fn exit_code_when_internal_error_then_3() {
        assert_eq!(Trap::StackOverflow.exit_code(), 3);
        assert_eq!(Trap::StackUnderflow.exit_code(), 3);
        assert_eq!(Trap::InvalidInstruction(0).exit_code(), 3);
        assert_eq!(Trap::InvalidConstantIndex(0).exit_code(), 3);
        assert_eq!(Trap::InvalidVariableIndex(0).exit_code(), 3);
        assert_eq!(Trap::InvalidFunctionId(0).exit_code(), 3);
        assert_eq!(Trap::InvalidBuiltinFunction(0).exit_code(), 3);
        assert_eq!(Trap::InvalidFbTypeId(0).exit_code(), 3);
    }
}
