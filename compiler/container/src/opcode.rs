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

/// Call a built-in standard library function.
/// Operand: u16 function ID (little-endian).
/// Stack effect depends on the specific function.
pub const BUILTIN: u8 = 0xC4;

/// Return from the current function (void return).
pub const RET_VOID: u8 = 0xB5;

/// Built-in function IDs used with the BUILTIN opcode.
pub mod builtin {
    /// EXPT for 32-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    pub const EXPT_I32: u16 = 0x0340;
}
