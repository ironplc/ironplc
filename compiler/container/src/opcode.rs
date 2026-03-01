//! Bytecode opcode definitions shared between the compiler and VM.

/// Load a 32-bit integer constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_I32: u8 = 0x01;

/// Push I32 value 1 (boolean TRUE).
pub const LOAD_TRUE: u8 = 0x07;

/// Push I32 value 0 (boolean FALSE).
pub const LOAD_FALSE: u8 = 0x08;

/// Load a 32-bit integer from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_I32: u8 = 0x10;

/// Store a 32-bit integer to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_I32: u8 = 0x18;

/// Add two 32-bit integers (wrapping).
/// Pops two values, pushes their sum.
pub const ADD_I32: u8 = 0x30;

/// Subtract two 32-bit integers (wrapping).
/// Pops two values (b then a), pushes a - b.
pub const SUB_I32: u8 = 0x31;

/// Multiply two 32-bit integers (wrapping).
/// Pops two values, pushes their product.
pub const MUL_I32: u8 = 0x32;

/// Divide two 32-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a / b.
/// Traps on division by zero.
pub const DIV_I32: u8 = 0x33;

/// Modulo (remainder) of two 32-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a % b.
/// Traps on division by zero.
pub const MOD_I32: u8 = 0x34;

/// Negate a 32-bit integer (wrapping).
/// Pops one value, pushes its negation.
pub const NEG_I32: u8 = 0x35;

/// Compare two 32-bit integers for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0.
pub const EQ_I32: u8 = 0x68;

/// Compare two 32-bit integers for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0.
pub const NE_I32: u8 = 0x69;

/// Compare two signed 32-bit integers (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0.
pub const LT_I32: u8 = 0x6A;

/// Compare two signed 32-bit integers (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0.
pub const LE_I32: u8 = 0x6B;

/// Compare two signed 32-bit integers (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0.
pub const GT_I32: u8 = 0x6C;

/// Compare two signed 32-bit integers (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0.
pub const GE_I32: u8 = 0x6D;

/// Logical AND of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if both are non-zero, else 0.
pub const BOOL_AND: u8 = 0x54;

/// Logical OR of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if either is non-zero, else 0.
pub const BOOL_OR: u8 = 0x55;

/// Logical XOR of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if exactly one is non-zero, else 0.
pub const BOOL_XOR: u8 = 0x56;

/// Logical NOT of a value.
/// Pops one value, pushes 1 if it is zero, else 0.
pub const BOOL_NOT: u8 = 0x57;

// --- Bitwise opcodes (32-bit) ---

/// Bitwise AND of two 32-bit integers.
/// Pops two values (b then a), pushes a & b.
pub const BIT_AND_32: u8 = 0x58;

/// Bitwise OR of two 32-bit integers.
/// Pops two values (b then a), pushes a | b.
pub const BIT_OR_32: u8 = 0x59;

/// Bitwise XOR of two 32-bit integers.
/// Pops two values (b then a), pushes a ^ b.
pub const BIT_XOR_32: u8 = 0x5A;

/// Bitwise NOT of a 32-bit integer.
/// Pops one value, pushes !a.
pub const BIT_NOT_32: u8 = 0x5B;

// --- Bitwise opcodes (64-bit) ---

/// Bitwise AND of two 64-bit integers.
/// Pops two values (b then a), pushes a & b.
pub const BIT_AND_64: u8 = 0x60;

/// Bitwise OR of two 64-bit integers.
/// Pops two values (b then a), pushes a | b.
pub const BIT_OR_64: u8 = 0x61;

/// Bitwise XOR of two 64-bit integers.
/// Pops two values (b then a), pushes a ^ b.
pub const BIT_XOR_64: u8 = 0x62;

/// Bitwise NOT of a 64-bit integer.
/// Pops one value, pushes !a.
pub const BIT_NOT_64: u8 = 0x63;

/// Unconditional jump. Operand: i16 offset relative to next instruction.
pub const JMP: u8 = 0xB0;

/// Jump if top of stack is zero (FALSE). Operand: i16 offset. Pops condition.
pub const JMP_IF_NOT: u8 = 0xB2;

/// Call a built-in standard library function.
/// Operand: u16 function ID (little-endian).
/// Stack effect depends on the specific function.
pub const BUILTIN: u8 = 0xC4;

/// Return from the current function (void return).
pub const RET_VOID: u8 = 0xB5;

// --- Truncation opcodes ---

/// Truncate i32 to i8 range, then sign-extend back to i32.
/// `(v as i8) as i32` — wraps to -128..127.
pub const TRUNC_I8: u8 = 0x20;

/// Truncate i32 to u8 range, then zero-extend back to i32.
/// `(v as u8) as i32` — wraps to 0..255.
pub const TRUNC_U8: u8 = 0x21;

/// Truncate i32 to i16 range, then sign-extend back to i32.
/// `(v as i16) as i32` — wraps to -32768..32767.
pub const TRUNC_I16: u8 = 0x22;

/// Truncate i32 to u16 range, then zero-extend back to i32.
/// `(v as u16) as i32` — wraps to 0..65535.
pub const TRUNC_U16: u8 = 0x23;

// --- 64-bit load/store opcodes ---

/// Load a 64-bit integer constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_I64: u8 = 0x02;

/// Load a 32-bit float constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_F32: u8 = 0x03;

/// Load a 64-bit float constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_F64: u8 = 0x04;

/// Load a 64-bit integer from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_I64: u8 = 0x11;

/// Load a 32-bit float from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_F32: u8 = 0x12;

/// Load a 64-bit float from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_F64: u8 = 0x13;

/// Store a 64-bit integer to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_I64: u8 = 0x19;

/// Store a 32-bit float to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_F32: u8 = 0x1A;

/// Store a 64-bit float to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_F64: u8 = 0x1B;

// --- 64-bit arithmetic opcodes ---

/// Add two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_add(b).
pub const ADD_I64: u8 = 0x38;

/// Subtract two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_sub(b).
pub const SUB_I64: u8 = 0x39;

/// Multiply two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_mul(b).
pub const MUL_I64: u8 = 0x3A;

/// Divide two signed 64-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a / b. Traps on division by zero.
pub const DIV_I64: u8 = 0x3B;

/// Modulo (remainder) of two signed 64-bit integers.
/// Pops two values (b then a), pushes a % b. Traps on division by zero.
pub const MOD_I64: u8 = 0x3C;

/// Negate a 64-bit integer (wrapping).
/// Pops one value, pushes its negation.
pub const NEG_I64: u8 = 0x3D;

// --- Unsigned 32-bit division opcodes ---

/// Divide two unsigned 32-bit integers.
/// Pops two i32 values (b then a), reinterprets as u32, pushes (a/b) as i32.
/// Traps on division by zero.
pub const DIV_U32: u8 = 0x40;

/// Modulo (remainder) of two unsigned 32-bit integers.
/// Pops two i32 values (b then a), reinterprets as u32, pushes (a%b) as i32.
/// Traps on division by zero.
pub const MOD_U32: u8 = 0x41;

/// Divide two unsigned 64-bit integers.
/// Pops two i64 values (b then a), reinterprets as u64, pushes (a/b) as i64.
/// Traps on division by zero.
pub const DIV_U64: u8 = 0x42;

/// Modulo (remainder) of two unsigned 64-bit integers.
/// Pops two i64 values (b then a), reinterprets as u64, pushes (a%b) as i64.
/// Traps on division by zero.
pub const MOD_U64: u8 = 0x43;

// --- 32-bit float arithmetic opcodes ---

/// Add two 32-bit floats.
/// Pops two values (b then a), pushes a + b.
pub const ADD_F32: u8 = 0x48;

/// Subtract two 32-bit floats.
/// Pops two values (b then a), pushes a - b.
pub const SUB_F32: u8 = 0x49;

/// Multiply two 32-bit floats.
/// Pops two values (b then a), pushes a * b.
pub const MUL_F32: u8 = 0x4A;

/// Divide two 32-bit floats.
/// Pops two values (b then a), pushes a / b.
/// IEEE 754: produces ±Inf or NaN on division by zero.
pub const DIV_F32: u8 = 0x4B;

/// Negate a 32-bit float.
/// Pops one value, pushes its negation.
pub const NEG_F32: u8 = 0x4C;

// --- 64-bit float arithmetic opcodes ---

/// Add two 64-bit floats.
/// Pops two values (b then a), pushes a + b.
pub const ADD_F64: u8 = 0x4E;

/// Subtract two 64-bit floats.
/// Pops two values (b then a), pushes a - b.
pub const SUB_F64: u8 = 0x4F;

/// Multiply two 64-bit floats.
/// Pops two values (b then a), pushes a * b.
pub const MUL_F64: u8 = 0x50;

/// Divide two 64-bit floats.
/// Pops two values (b then a), pushes a / b.
/// IEEE 754: produces ±Inf or NaN on division by zero.
pub const DIV_F64: u8 = 0x51;

/// Negate a 64-bit float.
/// Pops one value, pushes its negation.
pub const NEG_F64: u8 = 0x52;

// --- 64-bit comparison opcodes ---

/// Compare two 64-bit integers for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0.
pub const EQ_I64: u8 = 0x70;

/// Compare two 64-bit integers for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0.
pub const NE_I64: u8 = 0x71;

/// Compare two signed 64-bit integers (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0.
pub const LT_I64: u8 = 0x72;

/// Compare two signed 64-bit integers (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0.
pub const LE_I64: u8 = 0x73;

/// Compare two signed 64-bit integers (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0.
pub const GT_I64: u8 = 0x74;

/// Compare two signed 64-bit integers (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0.
pub const GE_I64: u8 = 0x75;

// --- Unsigned comparison opcodes ---

/// Compare two unsigned 32-bit integers (less than).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) < (b as u32), else 0.
pub const LT_U32: u8 = 0x78;

/// Compare two unsigned 32-bit integers (less than or equal).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) <= (b as u32), else 0.
pub const LE_U32: u8 = 0x79;

/// Compare two unsigned 32-bit integers (greater than).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) > (b as u32), else 0.
pub const GT_U32: u8 = 0x7A;

/// Compare two unsigned 32-bit integers (greater than or equal).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) >= (b as u32), else 0.
pub const GE_U32: u8 = 0x7B;

/// Compare two unsigned 64-bit integers (less than).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) < (b as u64), else 0.
pub const LT_U64: u8 = 0x7C;

/// Compare two unsigned 64-bit integers (less than or equal).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) <= (b as u64), else 0.
pub const LE_U64: u8 = 0x7D;

/// Compare two unsigned 64-bit integers (greater than).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) > (b as u64), else 0.
pub const GT_U64: u8 = 0x7E;

/// Compare two unsigned 64-bit integers (greater than or equal).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) >= (b as u64), else 0.
pub const GE_U64: u8 = 0x7F;

// --- 32-bit float comparison opcodes ---

/// Compare two 32-bit floats for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
pub const EQ_F32: u8 = 0x80;

/// Compare two 32-bit floats for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
pub const NE_F32: u8 = 0x81;

/// Compare two 32-bit floats (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
pub const LT_F32: u8 = 0x82;

/// Compare two 32-bit floats (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
pub const LE_F32: u8 = 0x83;

/// Compare two 32-bit floats (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
pub const GT_F32: u8 = 0x84;

/// Compare two 32-bit floats (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
pub const GE_F32: u8 = 0x85;

// --- 64-bit float comparison opcodes ---

/// Compare two 64-bit floats for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
pub const EQ_F64: u8 = 0x88;

/// Compare two 64-bit floats for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
pub const NE_F64: u8 = 0x89;

/// Compare two 64-bit floats (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
pub const LT_F64: u8 = 0x8A;

/// Compare two 64-bit floats (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
pub const LE_F64: u8 = 0x8B;

/// Compare two 64-bit floats (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
pub const GT_F64: u8 = 0x8C;

/// Compare two 64-bit floats (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
pub const GE_F64: u8 = 0x8D;

/// Built-in function IDs used with the BUILTIN opcode.
pub mod builtin {
    /// EXPT for 32-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    pub const EXPT_I32: u16 = 0x0340;

    /// EXPT for 32-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    pub const EXPT_F32: u16 = 0x0341;

    /// EXPT for 64-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    pub const EXPT_F64: u16 = 0x0342;
}
