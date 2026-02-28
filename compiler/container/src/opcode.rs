//! Bytecode opcode definitions shared between the compiler and VM.

/// Load a 32-bit integer constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_I32: u8 = 0x01;

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

/// Return from the current function (void return).
pub const RET_VOID: u8 = 0xB5;
