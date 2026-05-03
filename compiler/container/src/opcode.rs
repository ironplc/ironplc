//! Bytecode opcode definitions shared between the compiler and VM.
//!
//! # Encoding
//!
//! Each `Opcode` is one byte, encoded as `[op_class:6][type:2]`:
//!
//! ```text
//!   bits:    7 6 5 4 3 2 1 0
//!            └──op_class──┘└type┘
//! ```
//!
//! - **op_class** (high 6 bits) selects the operation. 64 slots total.
//! - **type tag** (low 2 bits) selects the type variant or, for some
//!   op-classes, the operation within a small consolidated family
//!   (`LOAD_BOOL`, `BOOL_OP`, `STACK_OP`).
//!
//! Type-tag values: `T_I32 = 0`, `T_I64 = 1`, `T_F32 = 2`, `T_F64 = 3`.
//! Op-classes that use only the int subset use `T_I32`/`T_I64` and trap
//! on float type tags. Untyped op-classes (jumps, calls, single-variant
//! ops) require `type_tag = 0`.
//!
//! See `specs/design/bytecode-instruction-set.md` § Encoding for the
//! design rules (op-class = "what operation"; type-tag = "what kind of
//! data"; sub-opcode-in-operand = "which family member" for op classes
//! consolidating large families like STRING_OP).
//!
//! This file is being migrated from an ad-hoc encoding to the structured
//! encoding above in waves; opcodes whose definition still uses a raw
//! hex literal have not yet been moved. The helpers `encode_opcode` and
//! `decode_opcode` work for both old and new opcode bytes — they only
//! interpret the byte's bit layout.

/// A primary bytecode opcode (one byte).
pub type Opcode = u8;

// --- Type-tag values ---

/// Type tag for 32-bit integer (signed I32 or width-32 unsigned).
pub const T_I32: u8 = 0;
/// Type tag for 64-bit integer (signed I64 or width-64 unsigned).
pub const T_I64: u8 = 1;
/// Type tag for 32-bit IEEE-754 float.
pub const T_F32: u8 = 2;
/// Type tag for 64-bit IEEE-754 float.
pub const T_F64: u8 = 3;

// --- Op-class values (high 6 bits of the opcode byte) ---
//
// All 41 op classes are defined here even though some are not yet used
// to derive opcode bytes (incremental wave migration). Future waves
// reference these constants without redefining them.

/// Op class: load a constant from the constant pool. Type tag selects width.
pub const OP_CLASS_LOAD_CONST: u8 = 0x00;
/// Op class: push a boolean literal. Type tag *is* the value (0 = FALSE, 1 = TRUE).
pub const OP_CLASS_LOAD_BOOL: u8 = 0x01;
/// Op class: load a string literal from the constant pool.
pub const OP_CLASS_LOAD_CONST_STR: u8 = 0x02;
/// Op class: load a variable. Type tag selects slot width.
pub const OP_CLASS_LOAD_VAR: u8 = 0x03;
/// Op class: store to a variable. Type tag selects slot width.
pub const OP_CLASS_STORE_VAR: u8 = 0x04;
/// Op class: indirect load (dereference reference on stack).
pub const OP_CLASS_LOAD_INDIRECT: u8 = 0x05;
/// Op class: indirect store.
pub const OP_CLASS_STORE_INDIRECT: u8 = 0x06;
/// Op class: truncate to narrow integer width. Type tag selects target (I8/U8/I16/U16).
pub const OP_CLASS_TRUNC: u8 = 0x07;
/// Op class: arithmetic addition. Type tag selects width.
pub const OP_CLASS_ADD: u8 = 0x08;
/// Op class: arithmetic subtraction.
pub const OP_CLASS_SUB: u8 = 0x09;
/// Op class: arithmetic multiplication.
pub const OP_CLASS_MUL: u8 = 0x0A;
/// Op class: arithmetic negation.
pub const OP_CLASS_NEG: u8 = 0x0B;
/// Op class: signed division (and float division).
pub const OP_CLASS_DIV_S: u8 = 0x0C;
/// Op class: unsigned integer division. Only U32/U64 type variants.
pub const OP_CLASS_DIV_U: u8 = 0x0D;
/// Op class: signed integer modulo. Only I32/I64; floats have no MOD.
pub const OP_CLASS_MOD_S: u8 = 0x0E;
/// Op class: unsigned integer modulo. Only U32/U64.
pub const OP_CLASS_MOD_U: u8 = 0x0F;
/// Op class: equality comparison (sign-blind).
pub const OP_CLASS_EQ: u8 = 0x10;
/// Op class: inequality comparison (sign-blind).
pub const OP_CLASS_NE: u8 = 0x11;
/// Op class: signed less-than (and float less-than).
pub const OP_CLASS_LT_S: u8 = 0x12;
/// Op class: signed less-than-or-equal (and float).
pub const OP_CLASS_LE_S: u8 = 0x13;
/// Op class: signed greater-than (and float).
pub const OP_CLASS_GT_S: u8 = 0x14;
/// Op class: signed greater-than-or-equal (and float).
pub const OP_CLASS_GE_S: u8 = 0x15;
/// Op class: unsigned less-than. Only U32/U64.
pub const OP_CLASS_LT_U: u8 = 0x16;
/// Op class: unsigned less-than-or-equal.
pub const OP_CLASS_LE_U: u8 = 0x17;
/// Op class: unsigned greater-than.
pub const OP_CLASS_GT_U: u8 = 0x18;
/// Op class: unsigned greater-than-or-equal.
pub const OP_CLASS_GE_U: u8 = 0x19;
/// Op class: bitwise AND. Type tag 0 = W32, 1 = W64.
pub const OP_CLASS_BIT_AND: u8 = 0x1A;
/// Op class: bitwise OR.
pub const OP_CLASS_BIT_OR: u8 = 0x1B;
/// Op class: bitwise XOR.
pub const OP_CLASS_BIT_XOR: u8 = 0x1C;
/// Op class: bitwise NOT.
pub const OP_CLASS_BIT_NOT: u8 = 0x1D;
/// Op class: boolean operations (consolidated). Type tag selects: 0 = AND,
/// 1 = OR, 2 = XOR, 3 = NOT.
pub const OP_CLASS_BOOL_OP: u8 = 0x1E;
/// Op class: unconditional jump.
pub const OP_CLASS_JMP: u8 = 0x1F;
/// Op class: jump if top-of-stack is zero.
pub const OP_CLASS_JMP_IF_NOT: u8 = 0x20;
/// Op class: function call.
pub const OP_CLASS_CALL: u8 = 0x21;
/// Op class: function return with value.
pub const OP_CLASS_RET: u8 = 0x22;
/// Op class: function return without value.
pub const OP_CLASS_RET_VOID: u8 = 0x23;
/// Op class: stack manipulation (consolidated). Type tag selects:
/// 0 = POP, 1 = DUP, 2 = SWAP.
pub const OP_CLASS_STACK_OP: u8 = 0x24;
/// Op class: built-in standard-library function call.
pub const OP_CLASS_BUILTIN: u8 = 0x25;
/// Op class: load FB instance reference.
pub const OP_CLASS_FB_LOAD_INSTANCE: u8 = 0x26;
/// Op class: store FB input parameter.
pub const OP_CLASS_FB_STORE_PARAM: u8 = 0x27;
/// Op class: load FB output parameter.
pub const OP_CLASS_FB_LOAD_PARAM: u8 = 0x28;
/// Op class: invoke FB body.
pub const OP_CLASS_FB_CALL: u8 = 0x29;
/// Op class: load array element.
pub const OP_CLASS_LOAD_ARRAY: u8 = 0x2A;
/// Op class: store array element.
pub const OP_CLASS_STORE_ARRAY: u8 = 0x2B;
/// Op class: load array element via reference.
pub const OP_CLASS_LOAD_ARRAY_DEREF: u8 = 0x2C;
/// Op class: store array element via reference.
pub const OP_CLASS_STORE_ARRAY_DEREF: u8 = 0x2D;
/// Op class: STR_INIT.
pub const OP_CLASS_STR_INIT: u8 = 0x2E;
/// Op class: STR_LOAD_VAR.
pub const OP_CLASS_STR_LOAD_VAR: u8 = 0x2F;
/// Op class: STR_STORE_VAR.
pub const OP_CLASS_STR_STORE_VAR: u8 = 0x30;
/// Op class: LEN_STR.
pub const OP_CLASS_LEN_STR: u8 = 0x31;
/// Op class: FIND_STR.
pub const OP_CLASS_FIND_STR: u8 = 0x32;
/// Op class: REPLACE_STR.
pub const OP_CLASS_REPLACE_STR: u8 = 0x33;
/// Op class: INSERT_STR.
pub const OP_CLASS_INSERT_STR: u8 = 0x34;
/// Op class: DELETE_STR.
pub const OP_CLASS_DELETE_STR: u8 = 0x35;
/// Op class: LEFT_STR.
pub const OP_CLASS_LEFT_STR: u8 = 0x36;
/// Op class: RIGHT_STR.
pub const OP_CLASS_RIGHT_STR: u8 = 0x37;
/// Op class: MID_STR.
pub const OP_CLASS_MID_STR: u8 = 0x38;
/// Op class: CONCAT_STR.
pub const OP_CLASS_CONCAT_STR: u8 = 0x39;
/// Op class: STR_INIT_ARRAY.
pub const OP_CLASS_STR_INIT_ARRAY: u8 = 0x3A;
/// Op class: STR_LOAD_ARRAY_ELEM.
pub const OP_CLASS_STR_LOAD_ARRAY_ELEM: u8 = 0x3B;
/// Op class: STR_STORE_ARRAY_ELEM.
pub const OP_CLASS_STR_STORE_ARRAY_ELEM: u8 = 0x3C;
/// Op class: fused compare-and-branch. Type tag selects the type family
/// (`T_I32`/`T_I64`; floats reserved). The comparison operator is encoded
/// as a 1-byte operand (`cmp_op` enum). See `vm-performance.md` §11.
pub const OP_CLASS_CMP_BR: u8 = 0x3D;
// 0x3E..0x3F free (2 op-class slots reserved for future use).

/// Decompose a primary opcode byte into `(op_class, type_tag)`.
#[inline]
pub const fn decode_opcode(op: Opcode) -> (u8, u8) {
    (op >> 2, op & 0x03)
}

/// Compose `(op_class, type_tag)` into a primary opcode byte.
#[inline]
pub const fn encode_opcode(op_class: u8, type_tag: u8) -> Opcode {
    (op_class << 2) | (type_tag & 0x03)
}

/// Load a 32-bit integer constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_I32: Opcode = encode_opcode(OP_CLASS_LOAD_CONST, T_I32);

/// Push I32 value 1 (boolean TRUE). Encoded as `LOAD_BOOL` with type tag = 1.
pub const LOAD_TRUE: Opcode = encode_opcode(OP_CLASS_LOAD_BOOL, 1);

/// Push I32 value 0 (boolean FALSE). Encoded as `LOAD_BOOL` with type tag = 0.
pub const LOAD_FALSE: Opcode = encode_opcode(OP_CLASS_LOAD_BOOL, 0);

/// Load a 32-bit integer from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_I32: Opcode = encode_opcode(OP_CLASS_LOAD_VAR, T_I32);

/// Store a 32-bit integer to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_I32: Opcode = encode_opcode(OP_CLASS_STORE_VAR, T_I32);

/// Add two 32-bit integers (wrapping).
/// Pops two values, pushes their sum.
pub const ADD_I32: Opcode = encode_opcode(OP_CLASS_ADD, T_I32);

/// Subtract two 32-bit integers (wrapping).
/// Pops two values (b then a), pushes a - b.
pub const SUB_I32: Opcode = encode_opcode(OP_CLASS_SUB, T_I32);

/// Multiply two 32-bit integers (wrapping).
/// Pops two values, pushes their product.
pub const MUL_I32: Opcode = encode_opcode(OP_CLASS_MUL, T_I32);

/// Divide two 32-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a / b.
/// Traps on division by zero.
pub const DIV_I32: Opcode = encode_opcode(OP_CLASS_DIV_S, T_I32);

/// Modulo (remainder) of two 32-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a % b.
/// Traps on division by zero.
pub const MOD_I32: Opcode = encode_opcode(OP_CLASS_MOD_S, T_I32);

/// Negate a 32-bit integer (wrapping).
/// Pops one value, pushes its negation.
pub const NEG_I32: Opcode = encode_opcode(OP_CLASS_NEG, T_I32);

/// Compare two 32-bit integers for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0.
pub const EQ_I32: Opcode = encode_opcode(OP_CLASS_EQ, T_I32);

/// Compare two 32-bit integers for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0.
pub const NE_I32: Opcode = encode_opcode(OP_CLASS_NE, T_I32);

/// Compare two signed 32-bit integers (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0.
pub const LT_I32: Opcode = encode_opcode(OP_CLASS_LT_S, T_I32);

/// Compare two signed 32-bit integers (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0.
pub const LE_I32: Opcode = encode_opcode(OP_CLASS_LE_S, T_I32);

/// Compare two signed 32-bit integers (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0.
pub const GT_I32: Opcode = encode_opcode(OP_CLASS_GT_S, T_I32);

/// Compare two signed 32-bit integers (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0.
pub const GE_I32: Opcode = encode_opcode(OP_CLASS_GE_S, T_I32);

/// Logical AND of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if both are non-zero, else 0.
pub const BOOL_AND: Opcode = encode_opcode(OP_CLASS_BOOL_OP, 0);

/// Logical OR of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if either is non-zero, else 0.
pub const BOOL_OR: Opcode = encode_opcode(OP_CLASS_BOOL_OP, 1);

/// Logical XOR of two values.
/// Pops two values (b then a), coerces non-zero to 1, pushes 1 if exactly one is non-zero, else 0.
pub const BOOL_XOR: Opcode = encode_opcode(OP_CLASS_BOOL_OP, 2);

/// Logical NOT of a value.
/// Pops one value, pushes 1 if it is zero, else 0.
pub const BOOL_NOT: Opcode = encode_opcode(OP_CLASS_BOOL_OP, 3);

// --- Bitwise opcodes (32-bit) ---

/// Bitwise AND of two 32-bit integers.
/// Pops two values (b then a), pushes a & b.
pub const BIT_AND_32: Opcode = encode_opcode(OP_CLASS_BIT_AND, 0);

/// Bitwise OR of two 32-bit integers.
/// Pops two values (b then a), pushes a | b.
pub const BIT_OR_32: Opcode = encode_opcode(OP_CLASS_BIT_OR, 0);

/// Bitwise XOR of two 32-bit integers.
/// Pops two values (b then a), pushes a ^ b.
pub const BIT_XOR_32: Opcode = encode_opcode(OP_CLASS_BIT_XOR, 0);

/// Bitwise NOT of a 32-bit integer.
/// Pops one value, pushes !a.
pub const BIT_NOT_32: Opcode = encode_opcode(OP_CLASS_BIT_NOT, 0);

// --- Bitwise opcodes (64-bit) ---

/// Bitwise AND of two 64-bit integers.
/// Pops two values (b then a), pushes a & b.
pub const BIT_AND_64: Opcode = encode_opcode(OP_CLASS_BIT_AND, 1);

/// Bitwise OR of two 64-bit integers.
/// Pops two values (b then a), pushes a | b.
pub const BIT_OR_64: Opcode = encode_opcode(OP_CLASS_BIT_OR, 1);

/// Bitwise XOR of two 64-bit integers.
/// Pops two values (b then a), pushes a ^ b.
pub const BIT_XOR_64: Opcode = encode_opcode(OP_CLASS_BIT_XOR, 1);

/// Bitwise NOT of a 64-bit integer.
/// Pops one value, pushes !a.
pub const BIT_NOT_64: Opcode = encode_opcode(OP_CLASS_BIT_NOT, 1);

/// Unconditional jump. Operand: i16 offset relative to next instruction.
pub const JMP: Opcode = encode_opcode(OP_CLASS_JMP, 0);

/// Jump if top of stack is zero (FALSE). Operand: i16 offset. Pops condition.
pub const JMP_IF_NOT: Opcode = encode_opcode(OP_CLASS_JMP_IF_NOT, 0);

/// Call a built-in standard library function.
/// Operand: u16 function ID (little-endian).
/// Stack effect depends on the specific function.
pub const BUILTIN: Opcode = encode_opcode(OP_CLASS_BUILTIN, 0);

/// Call function by index. Pops arguments, executes function body,
/// pushes return value.
/// Operand: u16 function_id (little-endian).
pub const CALL: Opcode = encode_opcode(OP_CLASS_CALL, 0);

/// Return from function with a value on the stack.
pub const RET: Opcode = encode_opcode(OP_CLASS_RET, 0);

/// Return from the current function (void return).
pub const RET_VOID: Opcode = encode_opcode(OP_CLASS_RET_VOID, 0);

/// Discard the top value from the operand stack.
pub const POP: Opcode = encode_opcode(OP_CLASS_STACK_OP, 0);

/// Duplicate the top value on the operand stack.
/// Stack effect: [..., a] -> [..., a, a]
pub const DUP: Opcode = encode_opcode(OP_CLASS_STACK_OP, 1);

/// Swap the top two values on the operand stack.
/// Stack effect: [..., a, b] -> [..., b, a]
pub const SWAP: Opcode = encode_opcode(OP_CLASS_STACK_OP, 2);

// --- Function block opcodes ---

/// Push FB instance reference from variable table.
/// Operand: u16 variable index (little-endian).
pub const FB_LOAD_INSTANCE: Opcode = encode_opcode(OP_CLASS_FB_LOAD_INSTANCE, 0);

/// Store input parameter on FB instance; keeps fb_ref on stack.
/// Operand: u8 field index.
pub const FB_STORE_PARAM: Opcode = encode_opcode(OP_CLASS_FB_STORE_PARAM, 0);

/// Load output parameter from FB instance; keeps fb_ref on stack.
/// Operand: u8 field index.
pub const FB_LOAD_PARAM: Opcode = encode_opcode(OP_CLASS_FB_LOAD_PARAM, 0);

/// Call function block (VM dispatches to intrinsic or bytecode body).
/// Operand: u16 type_id (little-endian).
pub const FB_CALL: Opcode = encode_opcode(OP_CLASS_FB_CALL, 0);

// --- String opcodes ---

/// Load a STRING literal from the constant pool into a temporary buffer.
/// Operand: u16 constant pool index (little-endian).
/// Pushes the temp buf_idx onto the stack.
pub const LOAD_CONST_STR: Opcode = encode_opcode(OP_CLASS_LOAD_CONST_STR, 0);

/// Initialize a STRING variable in the data region.
/// Operands: data_offset: u32, max_length: u16.
/// Sets max_length and cur_length=0 at the given data_offset.
pub const STR_INIT: Opcode = encode_opcode(OP_CLASS_STR_INIT, 0);

/// Copy STRING from data region into a temp buffer; push temp buf_idx.
/// Operand: data_offset: u32.
pub const STR_LOAD_VAR: Opcode = encode_opcode(OP_CLASS_STR_LOAD_VAR, 0);

/// Copy temp buffer contents into STRING variable at data_offset.
/// Operand: data_offset: u32. Pops buf_idx from stack.
pub const STR_STORE_VAR: Opcode = encode_opcode(OP_CLASS_STR_STORE_VAR, 0);

/// Read the current length of a STRING variable from the data region.
/// Operand: data_offset: u32.
/// Pushes the cur_length as an i32 onto the stack.
pub const LEN_STR: Opcode = encode_opcode(OP_CLASS_LEN_STR, 0);

/// Find the first occurrence of IN2 within IN1.
/// Operands: in1_data_offset: u32, in2_data_offset: u32.
/// Pushes the 1-based position as i32 (0 if not found).
pub const FIND_STR: Opcode = encode_opcode(OP_CLASS_FIND_STR, 0);

/// Replace L characters starting at position P in IN1 with IN2.
/// Operands: in1_data_offset: u32, in2_data_offset: u32.
/// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
pub const REPLACE_STR: Opcode = encode_opcode(OP_CLASS_REPLACE_STR, 0);

/// Insert IN2 into IN1 after position P.
/// Operands: in1_data_offset: u32, in2_data_offset: u32.
/// Pops P (i32) from stack. Pushes buf_idx (i32).
pub const INSERT_STR: Opcode = encode_opcode(OP_CLASS_INSERT_STR, 0);

/// Delete L characters from IN1 starting at position P.
/// Operand: in1_data_offset: u32.
/// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
pub const DELETE_STR: Opcode = encode_opcode(OP_CLASS_DELETE_STR, 0);

/// Return the leftmost L characters of IN.
/// Operand: in_data_offset: u32.
/// Pops L (i32) from stack. Pushes buf_idx (i32).
pub const LEFT_STR: Opcode = encode_opcode(OP_CLASS_LEFT_STR, 0);

/// Return the rightmost L characters of IN.
/// Operand: in_data_offset: u32.
/// Pops L (i32) from stack. Pushes buf_idx (i32).
pub const RIGHT_STR: Opcode = encode_opcode(OP_CLASS_RIGHT_STR, 0);

/// Return L characters from IN starting at position P.
/// Operand: in_data_offset: u32.
/// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
pub const MID_STR: Opcode = encode_opcode(OP_CLASS_MID_STR, 0);

/// Concatenate IN1 and IN2.
/// Operands: in1_data_offset: u32, in2_data_offset: u32.
/// Pushes buf_idx (i32).
pub const CONCAT_STR: Opcode = encode_opcode(OP_CLASS_CONCAT_STR, 0);

// --- String array opcodes ---

/// Initialize all string headers in an array of strings.
/// Operand 1: u16 variable table index (base data_offset).
/// Operand 2: u16 array descriptor index.
/// Uses element_extra from the descriptor as max_string_length.
/// Stack effect: none.
pub const STR_INIT_ARRAY: Opcode = encode_opcode(OP_CLASS_STR_INIT_ARRAY, 0);

/// Load a string from an array element into a temp buffer.
/// Operand 1: u16 variable table index (base data_offset).
/// Operand 2: u16 array descriptor index.
/// Pops flat_index, pushes buf_idx. Net stack: 0.
pub const STR_LOAD_ARRAY_ELEM: Opcode = encode_opcode(OP_CLASS_STR_LOAD_ARRAY_ELEM, 0);

/// Store a temp buffer into an array element's string slot.
/// Operand 1: u16 variable table index (base data_offset).
/// Operand 2: u16 array descriptor index.
/// Pops flat_index, then pops buf_idx. Net stack: -2.
pub const STR_STORE_ARRAY_ELEM: Opcode = encode_opcode(OP_CLASS_STR_STORE_ARRAY_ELEM, 0);

// --- Array opcodes ---

/// Load a value from an array element.
/// Operand 1: u16 variable table index (little-endian).
/// Operand 2: u16 array descriptor index (little-endian).
/// Pops 1 (flat index), pushes 1 (element value). Net stack: 0.
pub const LOAD_ARRAY: Opcode = encode_opcode(OP_CLASS_LOAD_ARRAY, 0);

/// Store a value to an array element.
/// Operand 1: u16 variable table index (little-endian).
/// Operand 2: u16 array descriptor index (little-endian).
/// Pops 2 (value, flat index). Net stack: -2.
pub const STORE_ARRAY: Opcode = encode_opcode(OP_CLASS_STORE_ARRAY, 0);

/// Load a value from an array element through a reference (double indirection).
/// Operand 1: u16 reference variable index (little-endian). The slot holds the
///            target array's variable index.
/// Operand 2: u16 array descriptor index (little-endian).
/// Pops 1 (flat index), pushes 1 (element value). Net stack: 0.
pub const LOAD_ARRAY_DEREF: Opcode = encode_opcode(OP_CLASS_LOAD_ARRAY_DEREF, 0);

/// Store a value to an array element through a reference (double indirection).
/// Operand 1: u16 reference variable index (little-endian). The slot holds the
///            target array's variable index.
/// Operand 2: u16 array descriptor index (little-endian).
/// Pops 2 (value, flat index). Net stack: -2.
pub const STORE_ARRAY_DEREF: Opcode = encode_opcode(OP_CLASS_STORE_ARRAY_DEREF, 0);

// --- Truncation opcodes ---

/// Truncate i32 to i8 range, then sign-extend back to i32.
/// `(v as i8) as i32` — wraps to -128..127.
pub const TRUNC_I8: Opcode = encode_opcode(OP_CLASS_TRUNC, 0);

/// Truncate i32 to u8 range, then zero-extend back to i32.
/// `(v as u8) as i32` — wraps to 0..255.
pub const TRUNC_U8: Opcode = encode_opcode(OP_CLASS_TRUNC, 1);

/// Truncate i32 to i16 range, then sign-extend back to i32.
/// `(v as i16) as i32` — wraps to -32768..32767.
pub const TRUNC_I16: Opcode = encode_opcode(OP_CLASS_TRUNC, 2);

/// Truncate i32 to u16 range, then zero-extend back to i32.
/// `(v as u16) as i32` — wraps to 0..65535.
pub const TRUNC_U16: Opcode = encode_opcode(OP_CLASS_TRUNC, 3);

// --- 64-bit load/store opcodes ---

/// Load a 64-bit integer constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_I64: Opcode = encode_opcode(OP_CLASS_LOAD_CONST, T_I64);

/// Load a 32-bit float constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_F32: Opcode = encode_opcode(OP_CLASS_LOAD_CONST, T_F32);

/// Load a 64-bit float constant from the constant pool.
/// Operand: u16 constant pool index (little-endian).
pub const LOAD_CONST_F64: Opcode = encode_opcode(OP_CLASS_LOAD_CONST, T_F64);

/// Load a 64-bit integer from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_I64: Opcode = encode_opcode(OP_CLASS_LOAD_VAR, T_I64);

/// Load a 32-bit float from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_F32: Opcode = encode_opcode(OP_CLASS_LOAD_VAR, T_F32);

/// Load a 64-bit float from the variable table.
/// Operand: u16 variable index (little-endian).
pub const LOAD_VAR_F64: Opcode = encode_opcode(OP_CLASS_LOAD_VAR, T_F64);

/// Indirect load: pops a reference (variable index) from the stack,
/// loads the referenced variable's value, and pushes it.
/// No operand. Stack: [..., ref] → [..., value].
pub const LOAD_INDIRECT: Opcode = encode_opcode(OP_CLASS_LOAD_INDIRECT, 0);

/// Indirect store: pops a value and a reference (variable index) from the stack,
/// stores the value into the referenced variable.
/// No operand. Stack: [..., value, ref] → [...].
pub const STORE_INDIRECT: Opcode = encode_opcode(OP_CLASS_STORE_INDIRECT, 0);

/// Store a 64-bit integer to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_I64: Opcode = encode_opcode(OP_CLASS_STORE_VAR, T_I64);

/// Store a 32-bit float to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_F32: Opcode = encode_opcode(OP_CLASS_STORE_VAR, T_F32);

/// Store a 64-bit float to the variable table.
/// Operand: u16 variable index (little-endian).
pub const STORE_VAR_F64: Opcode = encode_opcode(OP_CLASS_STORE_VAR, T_F64);

// --- 64-bit arithmetic opcodes ---

/// Add two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_add(b).
pub const ADD_I64: Opcode = encode_opcode(OP_CLASS_ADD, T_I64);

/// Subtract two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_sub(b).
pub const SUB_I64: Opcode = encode_opcode(OP_CLASS_SUB, T_I64);

/// Multiply two 64-bit integers (wrapping).
/// Pops two values (b then a), pushes a.wrapping_mul(b).
pub const MUL_I64: Opcode = encode_opcode(OP_CLASS_MUL, T_I64);

/// Divide two signed 64-bit integers (truncating toward zero).
/// Pops two values (b then a), pushes a / b. Traps on division by zero.
pub const DIV_I64: Opcode = encode_opcode(OP_CLASS_DIV_S, T_I64);

/// Modulo (remainder) of two signed 64-bit integers.
/// Pops two values (b then a), pushes a % b. Traps on division by zero.
pub const MOD_I64: Opcode = encode_opcode(OP_CLASS_MOD_S, T_I64);

/// Negate a 64-bit integer (wrapping).
/// Pops one value, pushes its negation.
pub const NEG_I64: Opcode = encode_opcode(OP_CLASS_NEG, T_I64);

// --- Unsigned 32-bit division opcodes ---

/// Divide two unsigned 32-bit integers.
/// Pops two i32 values (b then a), reinterprets as u32, pushes (a/b) as i32.
/// Traps on division by zero.
pub const DIV_U32: Opcode = encode_opcode(OP_CLASS_DIV_U, T_I32);

/// Modulo (remainder) of two unsigned 32-bit integers.
/// Pops two i32 values (b then a), reinterprets as u32, pushes (a%b) as i32.
/// Traps on division by zero.
pub const MOD_U32: Opcode = encode_opcode(OP_CLASS_MOD_U, T_I32);

/// Divide two unsigned 64-bit integers.
/// Pops two i64 values (b then a), reinterprets as u64, pushes (a/b) as i64.
/// Traps on division by zero.
pub const DIV_U64: Opcode = encode_opcode(OP_CLASS_DIV_U, T_I64);

/// Modulo (remainder) of two unsigned 64-bit integers.
/// Pops two i64 values (b then a), reinterprets as u64, pushes (a%b) as i64.
/// Traps on division by zero.
pub const MOD_U64: Opcode = encode_opcode(OP_CLASS_MOD_U, T_I64);

// --- 32-bit float arithmetic opcodes ---

/// Add two 32-bit floats.
/// Pops two values (b then a), pushes a + b.
pub const ADD_F32: Opcode = encode_opcode(OP_CLASS_ADD, T_F32);

/// Subtract two 32-bit floats.
/// Pops two values (b then a), pushes a - b.
pub const SUB_F32: Opcode = encode_opcode(OP_CLASS_SUB, T_F32);

/// Multiply two 32-bit floats.
/// Pops two values (b then a), pushes a * b.
pub const MUL_F32: Opcode = encode_opcode(OP_CLASS_MUL, T_F32);

/// Divide two 32-bit floats.
/// Pops two values (b then a), pushes a / b.
/// IEEE 754: produces ±Inf or NaN on division by zero.
pub const DIV_F32: Opcode = encode_opcode(OP_CLASS_DIV_S, T_F32);

/// Negate a 32-bit float.
/// Pops one value, pushes its negation.
pub const NEG_F32: Opcode = encode_opcode(OP_CLASS_NEG, T_F32);

// --- 64-bit float arithmetic opcodes ---

/// Add two 64-bit floats.
/// Pops two values (b then a), pushes a + b.
pub const ADD_F64: Opcode = encode_opcode(OP_CLASS_ADD, T_F64);

/// Subtract two 64-bit floats.
/// Pops two values (b then a), pushes a - b.
pub const SUB_F64: Opcode = encode_opcode(OP_CLASS_SUB, T_F64);

/// Multiply two 64-bit floats.
/// Pops two values (b then a), pushes a * b.
pub const MUL_F64: Opcode = encode_opcode(OP_CLASS_MUL, T_F64);

/// Divide two 64-bit floats.
/// Pops two values (b then a), pushes a / b.
/// IEEE 754: produces ±Inf or NaN on division by zero.
pub const DIV_F64: Opcode = encode_opcode(OP_CLASS_DIV_S, T_F64);

/// Negate a 64-bit float.
/// Pops one value, pushes its negation.
pub const NEG_F64: Opcode = encode_opcode(OP_CLASS_NEG, T_F64);

// --- 64-bit comparison opcodes ---

/// Compare two 64-bit integers for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0.
pub const EQ_I64: Opcode = encode_opcode(OP_CLASS_EQ, T_I64);

/// Compare two 64-bit integers for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0.
pub const NE_I64: Opcode = encode_opcode(OP_CLASS_NE, T_I64);

/// Compare two signed 64-bit integers (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0.
pub const LT_I64: Opcode = encode_opcode(OP_CLASS_LT_S, T_I64);

/// Compare two signed 64-bit integers (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0.
pub const LE_I64: Opcode = encode_opcode(OP_CLASS_LE_S, T_I64);

/// Compare two signed 64-bit integers (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0.
pub const GT_I64: Opcode = encode_opcode(OP_CLASS_GT_S, T_I64);

/// Compare two signed 64-bit integers (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0.
pub const GE_I64: Opcode = encode_opcode(OP_CLASS_GE_S, T_I64);

// --- Unsigned comparison opcodes ---

/// Compare two unsigned 32-bit integers (less than).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) < (b as u32), else 0.
pub const LT_U32: Opcode = encode_opcode(OP_CLASS_LT_U, T_I32);

/// Compare two unsigned 32-bit integers (less than or equal).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) <= (b as u32), else 0.
pub const LE_U32: Opcode = encode_opcode(OP_CLASS_LE_U, T_I32);

/// Compare two unsigned 32-bit integers (greater than).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) > (b as u32), else 0.
pub const GT_U32: Opcode = encode_opcode(OP_CLASS_GT_U, T_I32);

/// Compare two unsigned 32-bit integers (greater than or equal).
/// Pops two i32 values (b then a), pushes 1 if (a as u32) >= (b as u32), else 0.
pub const GE_U32: Opcode = encode_opcode(OP_CLASS_GE_U, T_I32);

/// Compare two unsigned 64-bit integers (less than).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) < (b as u64), else 0.
pub const LT_U64: Opcode = encode_opcode(OP_CLASS_LT_U, T_I64);

/// Compare two unsigned 64-bit integers (less than or equal).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) <= (b as u64), else 0.
pub const LE_U64: Opcode = encode_opcode(OP_CLASS_LE_U, T_I64);

/// Compare two unsigned 64-bit integers (greater than).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) > (b as u64), else 0.
pub const GT_U64: Opcode = encode_opcode(OP_CLASS_GT_U, T_I64);

/// Compare two unsigned 64-bit integers (greater than or equal).
/// Pops two i64 values (b then a), pushes 1 if (a as u64) >= (b as u64), else 0.
pub const GE_U64: Opcode = encode_opcode(OP_CLASS_GE_U, T_I64);

// --- 32-bit float comparison opcodes ---

/// Compare two 32-bit floats for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
pub const EQ_F32: Opcode = encode_opcode(OP_CLASS_EQ, T_F32);

/// Compare two 32-bit floats for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
pub const NE_F32: Opcode = encode_opcode(OP_CLASS_NE, T_F32);

/// Compare two 32-bit floats (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
pub const LT_F32: Opcode = encode_opcode(OP_CLASS_LT_S, T_F32);

/// Compare two 32-bit floats (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
pub const LE_F32: Opcode = encode_opcode(OP_CLASS_LE_S, T_F32);

/// Compare two 32-bit floats (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
pub const GT_F32: Opcode = encode_opcode(OP_CLASS_GT_S, T_F32);

/// Compare two 32-bit floats (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
pub const GE_F32: Opcode = encode_opcode(OP_CLASS_GE_S, T_F32);

// --- 64-bit float comparison opcodes ---

/// Compare two 64-bit floats for equality.
/// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
pub const EQ_F64: Opcode = encode_opcode(OP_CLASS_EQ, T_F64);

/// Compare two 64-bit floats for inequality.
/// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
pub const NE_F64: Opcode = encode_opcode(OP_CLASS_NE, T_F64);

/// Compare two 64-bit floats (less than).
/// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
pub const LT_F64: Opcode = encode_opcode(OP_CLASS_LT_S, T_F64);

/// Compare two 64-bit floats (less than or equal).
/// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
pub const LE_F64: Opcode = encode_opcode(OP_CLASS_LE_S, T_F64);

/// Compare two 64-bit floats (greater than).
/// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
pub const GT_F64: Opcode = encode_opcode(OP_CLASS_GT_S, T_F64);

/// Compare two 64-bit floats (greater than or equal).
/// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
pub const GE_F64: Opcode = encode_opcode(OP_CLASS_GE_S, T_F64);

// --- Fused compare-and-branch opcodes ---

/// Fused compare-and-branch on 32-bit signed integers.
///
/// Operands:
/// - `cmp_op:u8` — comparison operator (see `cmp_op` module).
/// - `var_idx:u16` — variable index of the LHS.
/// - `const_idx:u16` — constant pool index of the RHS.
/// - `target:i16` — branch offset relative to the next instruction.
///
/// Semantics: load `cur = vars[var_idx]` and `cnst = const_pool[const_idx]`
/// directly (no stack), evaluate `cmp_op(cur, cnst)`. If the result is true,
/// add `target` to the program counter; otherwise fall through.
///
/// Stack effect: 0.
pub const CMP_BR_I32: Opcode = encode_opcode(OP_CLASS_CMP_BR, T_I32);

/// Fused compare-and-branch on 64-bit signed integers.
/// See `CMP_BR_I32` for operand layout and semantics.
pub const CMP_BR_I64: Opcode = encode_opcode(OP_CLASS_CMP_BR, T_I64);

/// Comparison-operator codes used as the first operand of `CMP_BR_*`.
///
/// Negation pairs (used by codegen to emit a "branch if false" predicate
/// from a "branch if true" opcode):
///   EQ ↔ NE,  LT_S ↔ GE_S,  LE_S ↔ GT_S.
///
/// Commutation pairs (used to rewrite `const <cmp> var` to `var <cmp> const`):
///   EQ ↔ EQ,  NE ↔ NE,  LT_S ↔ GT_S,  LE_S ↔ GE_S.
pub mod cmp_op {
    pub const EQ: u8 = 0;
    pub const NE: u8 = 1;
    pub const LT_S: u8 = 2;
    pub const LE_S: u8 = 3;
    pub const GT_S: u8 = 4;
    pub const GE_S: u8 = 5;

    /// Returns the negation of the given comparison operator (e.g.
    /// `LT_S` ↔ `GE_S`). Returns `None` for unrecognised codes.
    pub const fn negate(cmp_op: u8) -> Option<u8> {
        match cmp_op {
            EQ => Some(NE),
            NE => Some(EQ),
            LT_S => Some(GE_S),
            GE_S => Some(LT_S),
            LE_S => Some(GT_S),
            GT_S => Some(LE_S),
            _ => None,
        }
    }

    /// Returns the commutation of the given comparison operator
    /// (i.e. the operator equivalent under operand swap).
    /// Returns `None` for unrecognised codes.
    pub const fn commute(cmp_op: u8) -> Option<u8> {
        match cmp_op {
            EQ => Some(EQ),
            NE => Some(NE),
            LT_S => Some(GT_S),
            GT_S => Some(LT_S),
            LE_S => Some(GE_S),
            GE_S => Some(LE_S),
            _ => None,
        }
    }

    /// Whether `cmp_op` is a recognised comparison operator code.
    pub const fn is_valid(cmp_op: u8) -> bool {
        matches!(cmp_op, EQ | NE | LT_S | LE_S | GT_S | GE_S)
    }
}

/// Returns the total byte size of the instruction starting with `op`.
///
/// This is the single source of truth for instruction sizes, shared by both
/// the emitter and the optimizer. Keeping one function prevents the two from
/// diverging and producing misaligned instruction boundaries.
pub fn instruction_size(op: Opcode) -> usize {
    match op {
        // 1-byte: arithmetic, logic, comparison, unary, stack, control.
        ADD_I32 | SUB_I32 | MUL_I32 | DIV_I32 | MOD_I32 | NEG_I32 | ADD_I64 | SUB_I64 | MUL_I64
        | DIV_I64 | MOD_I64 | NEG_I64 | DIV_U32 | MOD_U32 | DIV_U64 | MOD_U64 | ADD_F32
        | SUB_F32 | MUL_F32 | DIV_F32 | NEG_F32 | ADD_F64 | SUB_F64 | MUL_F64 | DIV_F64
        | NEG_F64 | EQ_I32 | NE_I32 | LT_I32 | LE_I32 | GT_I32 | GE_I32 | EQ_I64 | NE_I64
        | LT_I64 | LE_I64 | GT_I64 | GE_I64 | LT_U32 | LE_U32 | GT_U32 | GE_U32 | LT_U64
        | LE_U64 | GT_U64 | GE_U64 | EQ_F32 | NE_F32 | LT_F32 | LE_F32 | GT_F32 | GE_F32
        | EQ_F64 | NE_F64 | LT_F64 | LE_F64 | GT_F64 | GE_F64 | BOOL_AND | BOOL_OR | BOOL_XOR
        | BOOL_NOT | BIT_AND_32 | BIT_OR_32 | BIT_XOR_32 | BIT_NOT_32 | BIT_AND_64 | BIT_OR_64
        | BIT_XOR_64 | BIT_NOT_64 | TRUNC_I8 | TRUNC_U8 | TRUNC_I16 | TRUNC_U16 | LOAD_INDIRECT
        | STORE_INDIRECT | LOAD_TRUE | LOAD_FALSE | POP | DUP | SWAP | RET | RET_VOID => 1,

        // 2-byte: opcode + u8 field index.
        FB_STORE_PARAM | FB_LOAD_PARAM => 2,

        // 3-byte: opcode + u16.
        LOAD_CONST_I32 | LOAD_CONST_I64 | LOAD_CONST_F32 | LOAD_CONST_F64 | LOAD_CONST_STR
        | LOAD_VAR_I32 | LOAD_VAR_I64 | LOAD_VAR_F32 | LOAD_VAR_F64 | STORE_VAR_I32
        | STORE_VAR_I64 | STORE_VAR_F32 | STORE_VAR_F64 | FB_LOAD_INSTANCE | FB_CALL | JMP
        | JMP_IF_NOT | BUILTIN => 3,

        // 5-byte: opcode + u16 + u16.
        CALL | LOAD_ARRAY | STORE_ARRAY | LOAD_ARRAY_DEREF | STORE_ARRAY_DEREF | STR_INIT_ARRAY
        | STR_LOAD_ARRAY_ELEM | STR_STORE_ARRAY_ELEM => 5,

        // 5-byte: opcode + u32.
        STR_LOAD_VAR | STR_STORE_VAR | LEN_STR | DELETE_STR | LEFT_STR | RIGHT_STR | MID_STR => 5,

        // 7-byte: opcode + u32 + u16.
        STR_INIT => 7,

        // 8-byte: opcode + u8 + u16 + u16 + i16.
        CMP_BR_I32 | CMP_BR_I64 => 8,

        // 9-byte: opcode + u32 + u32.
        FIND_STR | REPLACE_STR | INSERT_STR | CONCAT_STR => 9,

        // Unknown: advance by 1 byte to avoid infinite loops.
        _ => 1,
    }
}

/// Built-in function IDs used with the BUILTIN opcode.
pub mod builtin {
    /// EXPT for 32-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    pub const EXPT_I32: u16 = 0x0340;

    /// EXPT for 32-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    pub const EXPT_F32: u16 = 0x0341;

    /// EXPT for 64-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    pub const EXPT_F64: u16 = 0x0342;

    /// ABS for 32-bit integers: pops one value, pushes its absolute value (wrapping).
    pub const ABS_I32: u16 = 0x0343;

    /// MIN for 32-bit integers: pops two values (b then a), pushes min(a, b).
    pub const MIN_I32: u16 = 0x0344;

    /// MAX for 32-bit integers: pops two values (b then a), pushes max(a, b).
    pub const MAX_I32: u16 = 0x0345;

    /// LIMIT for 32-bit integers: pops mx, in, mn, pushes clamp(in, mn, mx).
    pub const LIMIT_I32: u16 = 0x0346;

    /// SEL for 32-bit integers: pops in1, in0, g, pushes in0 if g==0 else in1.
    pub const SEL_I32: u16 = 0x0347;

    /// SHL for 32-bit: pops shift count (n) and value (a), pushes a << n.
    pub const SHL_I32: u16 = 0x0348;

    /// SHL for 64-bit: pops shift count (n) and value (a), pushes a << n.
    pub const SHL_I64: u16 = 0x0349;

    /// SHR for 32-bit: pops shift count (n) and value (a), pushes a >> n (logical).
    pub const SHR_I32: u16 = 0x034A;

    /// SHR for 64-bit: pops shift count (n) and value (a), pushes a >> n (logical).
    pub const SHR_I64: u16 = 0x034B;

    /// ROL for 32-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
    pub const ROL_I32: u16 = 0x034C;

    /// ROL for 64-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
    pub const ROL_I64: u16 = 0x034D;

    /// ROR for 32-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
    pub const ROR_I32: u16 = 0x034E;

    /// ROR for 64-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
    pub const ROR_I64: u16 = 0x034F;

    /// ROL for 8-bit (BYTE): narrow rotate within 8 bits.
    pub const ROL_U8: u16 = 0x0350;

    /// ROL for 16-bit (WORD): narrow rotate within 16 bits.
    pub const ROL_U16: u16 = 0x0351;

    /// ROR for 8-bit (BYTE): narrow rotate within 8 bits.
    pub const ROR_U8: u16 = 0x0352;

    /// ROR for 16-bit (WORD): narrow rotate within 16 bits.
    pub const ROR_U16: u16 = 0x0353;

    /// ABS for 32-bit floats: pops one value, pushes its absolute value.
    pub const ABS_F32: u16 = 0x0354;

    /// ABS for 64-bit floats: pops one value, pushes its absolute value.
    pub const ABS_F64: u16 = 0x0355;

    /// MIN for 32-bit floats: pops two values (b then a), pushes min(a, b).
    pub const MIN_F32: u16 = 0x0356;

    /// MIN for 64-bit floats: pops two values (b then a), pushes min(a, b).
    pub const MIN_F64: u16 = 0x0357;

    /// MAX for 32-bit floats: pops two values (b then a), pushes max(a, b).
    pub const MAX_F32: u16 = 0x0358;

    /// MAX for 64-bit floats: pops two values (b then a), pushes max(a, b).
    pub const MAX_F64: u16 = 0x0359;

    /// LIMIT for 32-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
    pub const LIMIT_F32: u16 = 0x035A;

    /// LIMIT for 64-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
    pub const LIMIT_F64: u16 = 0x035B;

    /// SEL for 32-bit floats: pops in1, in0 (f32), g (i32), pushes in0 if g==0 else in1.
    pub const SEL_F32: u16 = 0x035C;

    /// SEL for 64-bit floats: pops in1, in0 (f64), g (i32), pushes in0 if g==0 else in1.
    pub const SEL_F64: u16 = 0x035D;

    /// SQRT for 32-bit floats: pops one value, pushes its square root.
    pub const SQRT_F32: u16 = 0x035E;

    /// SQRT for 64-bit floats: pops one value, pushes its square root.
    pub const SQRT_F64: u16 = 0x035F;

    /// EXPT for 64-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    pub const EXPT_I64: u16 = 0x0360;

    /// ABS for 64-bit integers: pops one value, pushes its absolute value (wrapping).
    pub const ABS_I64: u16 = 0x0361;

    /// MIN for 64-bit signed integers: pops two values (b then a), pushes min(a, b).
    pub const MIN_I64: u16 = 0x0362;

    /// MAX for 64-bit signed integers: pops two values (b then a), pushes max(a, b).
    pub const MAX_I64: u16 = 0x0363;

    /// LIMIT for 64-bit signed integers: pops mx, in, mn, pushes clamp(in, mn, mx).
    pub const LIMIT_I64: u16 = 0x0364;

    /// SEL for 64-bit values: pops in1, in0 (i64), g (i32), pushes in0 if g==0 else in1.
    pub const SEL_I64: u16 = 0x0365;

    /// MIN for 32-bit unsigned integers: pops two values (b then a), pushes unsigned min.
    pub const MIN_U32: u16 = 0x0366;

    /// MAX for 32-bit unsigned integers: pops two values (b then a), pushes unsigned max.
    pub const MAX_U32: u16 = 0x0367;

    /// LIMIT for 32-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
    pub const LIMIT_U32: u16 = 0x0368;

    /// MIN for 64-bit unsigned integers: pops two values (b then a), pushes unsigned min.
    pub const MIN_U64: u16 = 0x0369;

    /// MAX for 64-bit unsigned integers: pops two values (b then a), pushes unsigned max.
    pub const MAX_U64: u16 = 0x036A;

    /// LIMIT for 64-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
    pub const LIMIT_U64: u16 = 0x036B;

    /// LN for 32-bit floats: pops one value, pushes its natural logarithm.
    pub const LN_F32: u16 = 0x036C;

    /// LN for 64-bit floats: pops one value, pushes its natural logarithm.
    pub const LN_F64: u16 = 0x036D;

    /// LOG for 32-bit floats: pops one value, pushes its base-10 logarithm.
    pub const LOG_F32: u16 = 0x036E;

    /// LOG for 64-bit floats: pops one value, pushes its base-10 logarithm.
    pub const LOG_F64: u16 = 0x036F;

    /// EXP for 32-bit floats: pops one value, pushes e raised to that power.
    pub const EXP_F32: u16 = 0x0370;

    /// EXP for 64-bit floats: pops one value, pushes e raised to that power.
    pub const EXP_F64: u16 = 0x0371;

    /// SIN for 32-bit floats: pops one value (radians), pushes its sine.
    pub const SIN_F32: u16 = 0x0372;

    /// SIN for 64-bit floats: pops one value (radians), pushes its sine.
    pub const SIN_F64: u16 = 0x0373;

    /// COS for 32-bit floats: pops one value (radians), pushes its cosine.
    pub const COS_F32: u16 = 0x0374;

    /// COS for 64-bit floats: pops one value (radians), pushes its cosine.
    pub const COS_F64: u16 = 0x0375;

    /// TAN for 32-bit floats: pops one value (radians), pushes its tangent.
    pub const TAN_F32: u16 = 0x0376;

    /// TAN for 64-bit floats: pops one value (radians), pushes its tangent.
    pub const TAN_F64: u16 = 0x0377;

    /// ASIN for 32-bit floats: pops one value, pushes its arc sine (radians).
    pub const ASIN_F32: u16 = 0x0378;

    /// ASIN for 64-bit floats: pops one value, pushes its arc sine (radians).
    pub const ASIN_F64: u16 = 0x0379;

    /// ACOS for 32-bit floats: pops one value, pushes its arc cosine (radians).
    pub const ACOS_F32: u16 = 0x037A;

    /// ACOS for 64-bit floats: pops one value, pushes its arc cosine (radians).
    pub const ACOS_F64: u16 = 0x037B;

    /// ATAN for 32-bit floats: pops one value, pushes its arc tangent (radians).
    pub const ATAN_F32: u16 = 0x037C;

    /// ATAN for 64-bit floats: pops one value, pushes its arc tangent (radians).
    pub const ATAN_F64: u16 = 0x037D;

    // --- Type conversion opcodes ---

    /// Convert signed 32-bit integer to 32-bit float.
    pub const CONV_I32_TO_F32: u16 = 0x037E;

    /// Convert signed 32-bit integer to 64-bit float.
    pub const CONV_I32_TO_F64: u16 = 0x037F;

    /// Convert signed 64-bit integer to 32-bit float.
    pub const CONV_I64_TO_F32: u16 = 0x0380;

    /// Convert signed 64-bit integer to 64-bit float.
    pub const CONV_I64_TO_F64: u16 = 0x0381;

    /// Convert unsigned 32-bit integer to 32-bit float.
    pub const CONV_U32_TO_F32: u16 = 0x0382;

    /// Convert unsigned 32-bit integer to 64-bit float.
    pub const CONV_U32_TO_F64: u16 = 0x0383;

    /// Convert unsigned 64-bit integer to 32-bit float.
    pub const CONV_U64_TO_F32: u16 = 0x0384;

    /// Convert unsigned 64-bit integer to 64-bit float.
    pub const CONV_U64_TO_F64: u16 = 0x0385;

    /// Convert 32-bit float to signed 32-bit integer (truncating).
    pub const CONV_F32_TO_I32: u16 = 0x0386;

    /// Convert 32-bit float to signed 64-bit integer (truncating).
    pub const CONV_F32_TO_I64: u16 = 0x0387;

    /// Convert 64-bit float to signed 32-bit integer (truncating).
    pub const CONV_F64_TO_I32: u16 = 0x0388;

    /// Convert 64-bit float to signed 64-bit integer (truncating).
    pub const CONV_F64_TO_I64: u16 = 0x0389;

    /// Convert 32-bit float to unsigned 32-bit integer (truncating).
    pub const CONV_F32_TO_U32: u16 = 0x038A;

    /// Convert 32-bit float to unsigned 64-bit integer (truncating).
    pub const CONV_F32_TO_U64: u16 = 0x038B;

    /// Convert 64-bit float to unsigned 32-bit integer (truncating).
    pub const CONV_F64_TO_U32: u16 = 0x038C;

    /// Convert 64-bit float to unsigned 64-bit integer (truncating).
    pub const CONV_F64_TO_U64: u16 = 0x038D;

    /// Widen 32-bit float to 64-bit float.
    pub const CONV_F32_TO_F64: u16 = 0x038E;

    /// Narrow 64-bit float to 32-bit float.
    pub const CONV_F64_TO_F32: u16 = 0x038F;

    /// Zero-extend unsigned 32-bit integer to 64-bit integer.
    pub const CONV_U32_TO_I64: u16 = 0x0390;

    // --- BCD conversion opcodes ---

    /// BCD_TO_INT for 8-bit (BYTE → USINT): decode 2 BCD digits.
    pub const BCD_TO_INT_8: u16 = 0x0391;

    /// BCD_TO_INT for 16-bit (WORD → UINT): decode 4 BCD digits.
    pub const BCD_TO_INT_16: u16 = 0x0392;

    /// BCD_TO_INT for 32-bit (DWORD → UDINT): decode 8 BCD digits.
    pub const BCD_TO_INT_32: u16 = 0x0393;

    /// BCD_TO_INT for 64-bit (LWORD → ULINT): decode 16 BCD digits.
    pub const BCD_TO_INT_64: u16 = 0x0394;

    /// INT_TO_BCD for 8-bit (USINT → BYTE): encode 2 BCD digits.
    pub const INT_TO_BCD_8: u16 = 0x0395;

    /// INT_TO_BCD for 16-bit (UINT → WORD): encode 4 BCD digits.
    pub const INT_TO_BCD_16: u16 = 0x0396;

    /// INT_TO_BCD for 32-bit (UDINT → DWORD): encode 8 BCD digits.
    pub const INT_TO_BCD_32: u16 = 0x0397;

    /// INT_TO_BCD for 64-bit (ULINT → LWORD): encode 16 BCD digits.
    pub const INT_TO_BCD_64: u16 = 0x0398;

    // --- Integer to boolean conversion opcodes ---

    /// Convert 32-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
    pub const CONV_I32_TO_BOOL: u16 = 0x0399;

    /// Convert 64-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
    pub const CONV_I64_TO_BOOL: u16 = 0x039A;

    // --- Two-argument trigonometric opcodes ---

    /// ATAN2 for 32-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
    pub const ATAN2_F32: u16 = 0x039B;

    /// ATAN2 for 64-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
    pub const ATAN2_F64: u16 = 0x039C;

    // =========================================================================
    // Numeric ↔ STRING conversion builtins
    //
    // These are dispatched inline in the VM main loop (not via
    // builtin::dispatch) because they need access to temp buffers and
    // the data region.
    // =========================================================================

    /// Convert signed 32-bit integer to decimal string.
    /// Stack: pop i32, push buf_idx (temp buffer with result).
    pub const CONV_I32_TO_STR: u16 = 0x039D;

    /// Convert unsigned 32-bit integer to decimal string.
    /// Stack: pop i32 (treated as u32), push buf_idx.
    pub const CONV_U32_TO_STR: u16 = 0x039E;

    /// Parse decimal string to signed 32-bit integer.
    /// Stack: pop data_offset (i32), push parsed i32 (0 on failure).
    pub const CONV_STR_TO_I32: u16 = 0x039F;

    /// Convert 32-bit float to decimal string.
    /// Stack: pop f32, push buf_idx (temp buffer with result).
    pub const CONV_F32_TO_STR: u16 = 0x03A0;

    /// Parse decimal string to 32-bit float.
    /// Stack: pop data_offset (i32), push parsed f32 (0.0 on failure).
    pub const CONV_STR_TO_F32: u16 = 0x03A1;

    /// Three-way lexicographic string comparison.
    /// Pops right_data_offset (i32) then left_data_offset (i32).
    /// Pushes -1 (left < right), 0 (equal), or +1 (left > right) as i32.
    pub const CMP_STR: u16 = 0x03A2;

    // =========================================================================
    // MUX (multiplexer) range-based opcodes
    //
    // MUX is extensible: the number of IN arguments varies per call site.
    // The func_id encodes the arity: BASE + n, where n is the number of
    // IN arguments (2..16). Total stack args = n + 1 (n IN values + K selector).
    // =========================================================================

    /// Base opcode for MUX with 32-bit signed integer values.
    /// MUX_I32_BASE + n = MUX with n IN arguments (n = 2..16).
    pub const MUX_I32_BASE: u16 = 0x0400;

    /// Base opcode for MUX with 64-bit signed integer values.
    pub const MUX_I64_BASE: u16 = 0x0420;

    /// Base opcode for MUX with 32-bit float values.
    pub const MUX_F32_BASE: u16 = 0x0440;

    /// Base opcode for MUX with 64-bit float values.
    pub const MUX_F64_BASE: u16 = 0x0460;

    /// Maximum number of IN arguments for MUX.
    pub const MUX_MAX_INPUTS: u16 = 16;

    /// Returns true if the given func_id is a MUX opcode.
    pub fn is_mux(func_id: u16) -> bool {
        mux_info(func_id).is_some()
    }

    /// Returns the number of IN arguments for a MUX opcode, or None if not a MUX opcode.
    pub fn mux_info(func_id: u16) -> Option<u16> {
        let bases = [MUX_I32_BASE, MUX_I64_BASE, MUX_F32_BASE, MUX_F64_BASE];
        for base in bases {
            if func_id >= base && func_id < base + MUX_MAX_INPUTS + 1 {
                let n = func_id - base;
                if n >= 2 {
                    return Some(n);
                }
            }
        }
        None
    }

    /// Returns the number of arguments a built-in function pops from the stack.
    ///
    /// This is the single source of truth for argument counts, used by both
    /// the codegen emitter (for stack depth tracking) and can be validated
    /// against the VM dispatch implementation.
    ///
    /// Panics if `func_id` is not a known built-in function ID.
    pub fn arg_count(func_id: u16) -> u16 {
        match func_id {
            ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64 | LN_F32 | LN_F64
            | LOG_F32 | LOG_F64 | EXP_F32 | EXP_F64 | SIN_F32 | SIN_F64 | COS_F32 | COS_F64
            | TAN_F32 | TAN_F64 | ASIN_F32 | ASIN_F64 | ACOS_F32 | ACOS_F64 | ATAN_F32
            | ATAN_F64 | CONV_I32_TO_F32 | CONV_I32_TO_F64 | CONV_I64_TO_F32 | CONV_I64_TO_F64
            | CONV_U32_TO_F32 | CONV_U32_TO_F64 | CONV_U64_TO_F32 | CONV_U64_TO_F64
            | CONV_F32_TO_I32 | CONV_F32_TO_I64 | CONV_F64_TO_I32 | CONV_F64_TO_I64
            | CONV_F32_TO_U32 | CONV_F32_TO_U64 | CONV_F64_TO_U32 | CONV_F64_TO_U64
            | CONV_F32_TO_F64 | CONV_F64_TO_F32 | CONV_U32_TO_I64 | BCD_TO_INT_8
            | BCD_TO_INT_16 | BCD_TO_INT_32 | BCD_TO_INT_64 | INT_TO_BCD_8 | INT_TO_BCD_16
            | INT_TO_BCD_32 | INT_TO_BCD_64 | CONV_I32_TO_BOOL | CONV_I64_TO_BOOL
            | CONV_I32_TO_STR | CONV_U32_TO_STR | CONV_STR_TO_I32 | CONV_F32_TO_STR
            | CONV_STR_TO_F32 => 1,
            EXPT_I32 | EXPT_F32 | EXPT_F64 | EXPT_I64 | MIN_I32 | MIN_F32 | MIN_F64 | MIN_I64
            | MIN_U32 | MIN_U64 | MAX_I32 | MAX_F32 | MAX_F64 | MAX_I64 | MAX_U32 | MAX_U64
            | SHL_I32 | SHL_I64 | SHR_I32 | SHR_I64 | ROL_I32 | ROL_I64 | ROR_I32 | ROR_I64
            | ROL_U8 | ROL_U16 | ROR_U8 | ROR_U16 | ATAN2_F32 | ATAN2_F64 | CMP_STR => 2,
            LIMIT_I32 | LIMIT_F32 | LIMIT_F64 | LIMIT_I64 | LIMIT_U32 | LIMIT_U64 | SEL_I32
            | SEL_F32 | SEL_F64 | SEL_I64 => 3,
            id if is_mux(id) => {
                // MUX pops n IN values + 1 K selector
                mux_info(id).unwrap() + 1
            }
            _ => panic!("unknown builtin function ID: 0x{:04X}", func_id),
        }
    }
}

/// Well-known function block type IDs for intrinsic dispatch.
pub mod fb_type {
    /// TON (on-delay timer).
    pub const TON: u16 = 0x0010;
    /// TOF (off-delay timer).
    pub const TOF: u16 = 0x0011;
    /// TP (pulse timer).
    pub const TP: u16 = 0x0012;
    /// CTU (count up counter).
    pub const CTU: u16 = 0x0020;
    /// CTD (count down counter).
    pub const CTD: u16 = 0x0021;
    /// CTUD (count up/down counter).
    pub const CTUD: u16 = 0x0022;
    /// SR (set-reset bistable, set dominant).
    pub const SR: u16 = 0x0030;
    /// RS (reset-set bistable, reset dominant).
    pub const RS: u16 = 0x0031;
    /// R_TRIG (rising edge detector).
    pub const R_TRIG: u16 = 0x0040;
    /// F_TRIG (falling edge detector).
    pub const F_TRIG: u16 = 0x0041;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_size_when_unknown_opcode_then_returns_one() {
        // 0xFE is not assigned; the default arm must return 1 so that
        // disassembly does not get stuck in an infinite loop.
        assert_eq!(instruction_size(0xFE), 1);
    }

    #[test]
    fn mux_info_when_valid_arity_then_returns_some_count() {
        assert_eq!(builtin::mux_info(builtin::MUX_I32_BASE + 3), Some(3));
        assert_eq!(builtin::mux_info(builtin::MUX_F64_BASE + 5), Some(5));
    }

    #[test]
    fn mux_info_when_arity_below_two_then_returns_none() {
        assert_eq!(builtin::mux_info(builtin::MUX_I32_BASE), None);
        assert_eq!(builtin::mux_info(builtin::MUX_I32_BASE + 1), None);
    }

    #[test]
    fn mux_info_when_not_a_mux_id_then_returns_none() {
        assert_eq!(builtin::mux_info(0x0001), None);
    }

    #[test]
    fn arg_count_when_mux_id_then_returns_n_plus_one() {
        // MUX pops n IN values + 1 K selector.
        assert_eq!(builtin::arg_count(builtin::MUX_I32_BASE + 3), 4);
    }

    #[test]
    #[should_panic(expected = "unknown builtin function ID")]
    fn arg_count_when_unknown_function_id_then_panics() {
        let _ = builtin::arg_count(0xFFFF);
    }
}
