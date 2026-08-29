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
    /// A `COPY_REGION` named a destination and a source whose array
    /// descriptors describe spans of different byte sizes.
    ///
    /// The copy length is derived from the descriptors rather than carried as
    /// an operand, so this is the check that a whole-aggregate assignment is
    /// moving like for like. Today the analyzer proves the two declared types
    /// are identical before codegen emits the instruction, so reaching this
    /// trap indicates a compiler defect. It is defined as a size disagreement
    /// rather than as an internal error because a future variable-length array
    /// (`ARRAY[*]`) has extents the analyzer cannot compare statically, at
    /// which point a correct compiler can produce this from a correct program.
    RegionSizeMismatch {
        dst_bytes: u32,
        src_bytes: u32,
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
            Trap::RegionSizeMismatch {
                dst_bytes,
                src_bytes,
            } => {
                write!(
                    f,
                    "region copy size mismatch: destination is {dst_bytes} bytes, source is {src_bytes} bytes"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Trap::DivideByZero, "divide by zero")]
    #[case(Trap::StackOverflow, "stack overflow")]
    #[case(Trap::StackUnderflow, "stack underflow")]
    #[case(Trap::InvalidInstruction(0xAB), "invalid instruction: 0xAB")]
    #[case(
        Trap::InvalidConstantIndex(ConstantIndex::new(5)),
        "invalid constant index: 5"
    )]
    #[case(
        Trap::InvalidVariableIndex(VarIndex::new(7)),
        "invalid variable index: 7"
    )]
    #[case(Trap::InvalidFunctionId(FunctionId::new(3)), "invalid function ID: 3")]
    #[case(Trap::WatchdogTimeout(TaskId::new(3)), "watchdog timeout on task 3")]
    #[case(Trap::NegativeExponent, "negative exponent")]
    #[case(Trap::NullDereference, "null reference dereference")]
    #[case(
        Trap::InvalidBuiltinFunction(FunctionId::new(0x0101)),
        "invalid built-in function: 0x0101"
    )]
    #[case(
        Trap::DataRegionOutOfBounds(42),
        "data region access out of bounds at offset 42"
    )]
    #[case(Trap::TempBufferExhausted, "temporary buffer pool exhausted")]
    #[case(
        Trap::InvalidFbTypeId(FbTypeId::new(0x0010)),
        "invalid FB type ID: 0x0010"
    )]
    #[case(
        Trap::ArrayIndexOutOfBounds {
            var_index: VarIndex::new(2),
            index: 10,
            total_elements: 5,
        },
        "array index out of bounds: index 10 for array variable 2 with 5 elements"
    )]
    #[case(Trap::UnexpectedEndOfBytecode, "bytecode ended mid-instruction")]
    #[case(Trap::CallStackOverflow, "call stack overflow")]
    #[case(Trap::InvalidCmpOp(0x07), "invalid comparison operator code: 0x07")]
    #[case(
        Trap::EncodingMismatch { expected: 1, actual: 2 },
        "string encoding mismatch: expected char_width 1, got 2"
    )]
    #[case(
        Trap::InvalidCharWidth(7),
        "invalid char_width byte: 7 (expected 1 or 2)"
    )]
    #[case(
        Trap::ProgramExceedsCallDepth { required: 64, capacity: 32 },
        "program declares call depth 64 but VM frame buffer holds at most 32"
    )]
    #[case(Trap::ZeroCallDepth, "container declares a maximum call depth of zero")]
    fn trap_display_when_variant_then_expected(#[case] trap: Trap, #[case] expected: &str) {
        assert_eq!(format!("{trap}"), expected);
    }

    #[rstest]
    #[case(Trap::DivideByZero, "V4001")]
    #[case(Trap::NegativeExponent, "V4002")]
    #[case(Trap::WatchdogTimeout(TaskId::new(0)), "V4003")]
    #[case(Trap::NullDereference, "V4004")]
    #[case(
        Trap::ArrayIndexOutOfBounds {
            var_index: VarIndex::new(0),
            index: 10,
            total_elements: 5,
        },
        "V4005"
    )]
    #[case(Trap::StackOverflow, "V9001")]
    #[case(Trap::StackUnderflow, "V9002")]
    #[case(Trap::InvalidInstruction(0xFF), "V9003")]
    #[case(Trap::InvalidConstantIndex(ConstantIndex::new(42)), "V9004")]
    #[case(Trap::InvalidVariableIndex(VarIndex::new(7)), "V9005")]
    #[case(Trap::InvalidFunctionId(FunctionId::new(3)), "V9006")]
    #[case(Trap::InvalidBuiltinFunction(FunctionId::new(0x0101)), "V9007")]
    #[case(Trap::InvalidFbTypeId(FbTypeId::new(0x0010)), "V9010")]
    #[case(Trap::UnexpectedEndOfBytecode, "V9011")]
    #[case(Trap::InvalidCmpOp(0xFF), "V9013")]
    #[case(Trap::EncodingMismatch { expected: 1, actual: 2 }, "V9014")]
    #[case(Trap::InvalidCharWidth(7), "V9015")]
    #[case(
        Trap::ProgramExceedsCallDepth { required: 64, capacity: 32 },
        "V9016"
    )]
    #[case(Trap::ZeroCallDepth, "V9017")]
    fn v_code_when_variant_then_expected(#[case] trap: Trap, #[case] expected: &str) {
        assert_eq!(trap.v_code(), expected);
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
