use core::fmt;

use ironplc_container::{ConstantIndex, FbTypeId, FunctionId, TaskId, VarIndex};

/// Runtime traps that halt VM execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    DivideByZero,
    StackOverflow,
    StackUnderflow,
    InvalidInstruction(u8),
    InvalidConstantIndex(ConstantIndex),
    InvalidVariableIndex(VarIndex),
    InvalidFunctionId(FunctionId),
    WatchdogTimeout(TaskId),
    NegativeExponent,
    NullDereference,
    InvalidBuiltinFunction(FunctionId),
    DataRegionOutOfBounds(u32),
    TempBufferExhausted,
    InvalidFbTypeId(FbTypeId),
    ArrayIndexOutOfBounds {
        var_index: VarIndex,
        index: i32,
        total_elements: u32,
    },
    UnexpectedEndOfBytecode,
    CallStackOverflow,
    InvalidCmpOp(u8),
    /// A string opcode encountered an operand whose `char_width` did not
    /// match the expected encoding. Per ADR-0034 the analyzer rejects
    /// cross-encoding operations statically; this trap is defense-in-depth
    /// against compiler bugs or tampered bytecode.
    EncodingMismatch {
        expected: u8,
        actual: u8,
    },
    /// A `char_width` byte read from a data-region header, temp buffer slot,
    /// constant-pool entry, or bytecode operand was neither `1` (STRING) nor
    /// `2` (WSTRING). The bytecode is malformed or has been tampered with.
    InvalidCharWidth(u8),
    /// The container's `header.max_call_depth` exceeds the frame-stack
    /// buffer provided by the embedder. The container was rejected at
    /// `VmReady::start` before any init code executed.
    ///
    /// `required` is the depth codegen declared the program needs;
    /// `capacity` is the size of the embedder's frame buffer. The
    /// embedder either grows the buffer or the program is recompiled
    /// with a shallower call chain.
    ProgramExceedsCallDepth {
        required: u16,
        capacity: u16,
    },
    /// The container's `header.max_call_depth` is zero. Every program needs
    /// at least one call frame for its entry function, so codegen always
    /// declares a depth of one or more; a zero means the field was never
    /// computed (a hand-built or legacy container). `VmReady::start`
    /// rejects it before any init code runs.
    ZeroCallDepth,
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
            Trap::NullDereference => write!(f, "null reference dereference"),
            Trap::InvalidBuiltinFunction(id) => {
                write!(f, "invalid built-in function: 0x{:04X}", id.raw())
            }
            Trap::DataRegionOutOfBounds(offset) => {
                write!(f, "data region access out of bounds at offset {offset}")
            }
            Trap::TempBufferExhausted => write!(f, "temporary buffer pool exhausted"),
            Trap::InvalidFbTypeId(id) => {
                write!(f, "invalid FB type ID: 0x{:04X}", id.raw())
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
            Trap::UnexpectedEndOfBytecode => write!(f, "bytecode ended mid-instruction"),
            Trap::CallStackOverflow => write!(f, "call stack overflow"),
            Trap::InvalidCmpOp(code) => write!(f, "invalid comparison operator code: 0x{code:02X}"),
            Trap::EncodingMismatch { expected, actual } => write!(
                f,
                "string encoding mismatch: expected char_width {expected}, got {actual}"
            ),
            Trap::InvalidCharWidth(value) => {
                write!(f, "invalid char_width byte: {value} (expected 1 or 2)")
            }
            Trap::ProgramExceedsCallDepth { required, capacity } => {
                write!(
                    f,
                    "program declares call depth {required} but VM frame buffer holds at most {capacity}"
                )
            }
            Trap::ZeroCallDepth => {
                write!(f, "container declares a maximum call depth of zero")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_display_when_divide_by_zero_then_expected() {
        assert_eq!(format!("{}", Trap::DivideByZero), "divide by zero");
    }

    #[test]
    fn trap_display_when_stack_overflow_then_expected() {
        assert_eq!(format!("{}", Trap::StackOverflow), "stack overflow");
    }

    #[test]
    fn trap_display_when_stack_underflow_then_expected() {
        assert_eq!(format!("{}", Trap::StackUnderflow), "stack underflow");
    }

    #[test]
    fn trap_display_when_invalid_instruction_then_includes_hex_opcode() {
        assert_eq!(
            format!("{}", Trap::InvalidInstruction(0xAB)),
            "invalid instruction: 0xAB"
        );
    }

    #[test]
    fn trap_display_when_invalid_constant_index_then_includes_index() {
        assert_eq!(
            format!("{}", Trap::InvalidConstantIndex(ConstantIndex::new(5))),
            "invalid constant index: 5"
        );
    }

    #[test]
    fn trap_display_when_invalid_variable_index_then_includes_index() {
        assert_eq!(
            format!("{}", Trap::InvalidVariableIndex(VarIndex::new(7))),
            "invalid variable index: 7"
        );
    }

    #[test]
    fn trap_display_when_invalid_function_id_then_includes_id() {
        assert_eq!(
            format!("{}", Trap::InvalidFunctionId(FunctionId::new(3))),
            "invalid function ID: 3"
        );
    }

    #[test]
    fn trap_display_when_watchdog_timeout_then_includes_task_id() {
        let trap = Trap::WatchdogTimeout(TaskId::new(3));
        assert_eq!(format!("{trap}"), "watchdog timeout on task 3");
    }

    #[test]
    fn trap_display_when_negative_exponent_then_expected() {
        assert_eq!(format!("{}", Trap::NegativeExponent), "negative exponent");
    }

    #[test]
    fn trap_display_when_null_dereference_then_expected() {
        assert_eq!(
            format!("{}", Trap::NullDereference),
            "null reference dereference"
        );
    }

    #[test]
    fn trap_display_when_invalid_builtin_function_then_includes_hex_id() {
        assert_eq!(
            format!("{}", Trap::InvalidBuiltinFunction(FunctionId::new(0x0101))),
            "invalid built-in function: 0x0101"
        );
    }

    #[test]
    fn trap_display_when_data_region_out_of_bounds_then_includes_offset() {
        assert_eq!(
            format!("{}", Trap::DataRegionOutOfBounds(42)),
            "data region access out of bounds at offset 42"
        );
    }

    #[test]
    fn trap_display_when_temp_buffer_exhausted_then_expected() {
        assert_eq!(
            format!("{}", Trap::TempBufferExhausted),
            "temporary buffer pool exhausted"
        );
    }

    #[test]
    fn trap_display_when_invalid_fb_type_id_then_includes_hex_id() {
        assert_eq!(
            format!("{}", Trap::InvalidFbTypeId(FbTypeId::new(0x0010))),
            "invalid FB type ID: 0x0010"
        );
    }

    #[test]
    fn trap_display_when_array_index_out_of_bounds_then_includes_details() {
        assert_eq!(
            format!(
                "{}",
                Trap::ArrayIndexOutOfBounds {
                    var_index: VarIndex::new(2),
                    index: 10,
                    total_elements: 5,
                }
            ),
            "array index out of bounds: index 10 for array variable 2 with 5 elements"
        );
    }

    #[test]
    fn trap_display_when_unexpected_end_of_bytecode_then_expected() {
        assert_eq!(
            format!("{}", Trap::UnexpectedEndOfBytecode),
            "bytecode ended mid-instruction"
        );
    }

    #[test]
    fn trap_display_when_call_stack_overflow_then_expected() {
        assert_eq!(
            format!("{}", Trap::CallStackOverflow),
            "call stack overflow"
        );
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
        assert_eq!(Trap::WatchdogTimeout(TaskId::new(0)).v_code(), "V4003");
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
        assert_eq!(
            Trap::InvalidConstantIndex(ConstantIndex::new(42)).v_code(),
            "V9004"
        );
    }

    #[test]
    fn v_code_when_invalid_variable_index_then_v9005() {
        assert_eq!(
            Trap::InvalidVariableIndex(VarIndex::new(7)).v_code(),
            "V9005"
        );
    }

    #[test]
    fn v_code_when_invalid_function_id_then_v9006() {
        assert_eq!(
            Trap::InvalidFunctionId(FunctionId::new(3)).v_code(),
            "V9006"
        );
    }

    #[test]
    fn v_code_when_invalid_builtin_function_then_v9007() {
        assert_eq!(
            Trap::InvalidBuiltinFunction(FunctionId::new(0x0101)).v_code(),
            "V9007"
        );
    }

    #[test]
    fn v_code_when_null_dereference_then_v4004() {
        assert_eq!(Trap::NullDereference.v_code(), "V4004");
    }

    #[test]
    fn v_code_when_array_index_out_of_bounds_then_v4005() {
        assert_eq!(
            Trap::ArrayIndexOutOfBounds {
                var_index: VarIndex::new(0),
                index: 10,
                total_elements: 5,
            }
            .v_code(),
            "V4005"
        );
    }

    #[test]
    fn v_code_when_invalid_fb_type_id_then_v9010() {
        assert_eq!(
            Trap::InvalidFbTypeId(FbTypeId::new(0x0010)).v_code(),
            "V9010"
        );
    }

    #[test]
    fn exit_code_when_user_error_then_1() {
        assert_eq!(Trap::DivideByZero.exit_code(), 1);
        assert_eq!(Trap::NegativeExponent.exit_code(), 1);
        assert_eq!(Trap::NullDereference.exit_code(), 1);
        assert_eq!(Trap::WatchdogTimeout(TaskId::new(0)).exit_code(), 1);
        assert_eq!(
            Trap::ArrayIndexOutOfBounds {
                var_index: VarIndex::new(0),
                index: 0,
                total_elements: 0,
            }
            .exit_code(),
            1
        );
    }

    #[test]
    fn v_code_when_unexpected_end_of_bytecode_then_v9011() {
        assert_eq!(Trap::UnexpectedEndOfBytecode.v_code(), "V9011");
    }

    #[test]
    fn trap_display_when_invalid_cmp_op_then_includes_hex_code() {
        assert_eq!(
            format!("{}", Trap::InvalidCmpOp(0x07)),
            "invalid comparison operator code: 0x07"
        );
    }

    #[test]
    fn v_code_when_invalid_cmp_op_then_v9013() {
        assert_eq!(Trap::InvalidCmpOp(0xFF).v_code(), "V9013");
    }

    #[test]
    fn trap_display_when_encoding_mismatch_then_includes_widths() {
        assert_eq!(
            format!(
                "{}",
                Trap::EncodingMismatch {
                    expected: 1,
                    actual: 2,
                }
            ),
            "string encoding mismatch: expected char_width 1, got 2"
        );
    }

    #[test]
    fn v_code_when_encoding_mismatch_then_v9014() {
        assert_eq!(
            Trap::EncodingMismatch {
                expected: 1,
                actual: 2,
            }
            .v_code(),
            "V9014"
        );
    }

    #[test]
    fn trap_display_when_invalid_char_width_then_includes_value() {
        assert_eq!(
            format!("{}", Trap::InvalidCharWidth(7)),
            "invalid char_width byte: 7 (expected 1 or 2)"
        );
    }

    #[test]
    fn v_code_when_invalid_char_width_then_v9015() {
        assert_eq!(Trap::InvalidCharWidth(7).v_code(), "V9015");
    }

    #[test]
    fn trap_display_when_program_exceeds_call_depth_then_includes_required_and_capacity() {
        assert_eq!(
            format!(
                "{}",
                Trap::ProgramExceedsCallDepth {
                    required: 64,
                    capacity: 32,
                }
            ),
            "program declares call depth 64 but VM frame buffer holds at most 32"
        );
    }

    #[test]
    fn v_code_when_program_exceeds_call_depth_then_v9016() {
        assert_eq!(
            Trap::ProgramExceedsCallDepth {
                required: 64,
                capacity: 32,
            }
            .v_code(),
            "V9016"
        );
    }

    #[test]
    fn trap_display_when_zero_call_depth_then_expected() {
        assert_eq!(
            format!("{}", Trap::ZeroCallDepth),
            "container declares a maximum call depth of zero"
        );
    }

    #[test]
    fn v_code_when_zero_call_depth_then_v9017() {
        assert_eq!(Trap::ZeroCallDepth.v_code(), "V9017");
    }

    #[test]
    fn exit_code_when_internal_error_then_3() {
        assert_eq!(Trap::StackOverflow.exit_code(), 3);
        assert_eq!(Trap::StackUnderflow.exit_code(), 3);
        assert_eq!(Trap::InvalidInstruction(0).exit_code(), 3);
        assert_eq!(
            Trap::InvalidConstantIndex(ConstantIndex::new(0)).exit_code(),
            3
        );
        assert_eq!(Trap::InvalidVariableIndex(VarIndex::new(0)).exit_code(), 3);
        assert_eq!(Trap::InvalidFunctionId(FunctionId::new(0)).exit_code(), 3);
        assert_eq!(
            Trap::InvalidBuiltinFunction(FunctionId::new(0)).exit_code(),
            3
        );
        assert_eq!(Trap::InvalidFbTypeId(FbTypeId::new(0)).exit_code(), 3);
        assert_eq!(Trap::UnexpectedEndOfBytecode.exit_code(), 3);
        assert_eq!(Trap::InvalidCmpOp(0).exit_code(), 3);
        assert_eq!(
            Trap::EncodingMismatch {
                expected: 1,
                actual: 2,
            }
            .exit_code(),
            3
        );
        assert_eq!(Trap::InvalidCharWidth(7).exit_code(), 3);
        assert_eq!(
            Trap::ProgramExceedsCallDepth {
                required: 64,
                capacity: 32,
            }
            .exit_code(),
            3
        );
        assert_eq!(Trap::ZeroCallDepth.exit_code(), 3);
    }
}
