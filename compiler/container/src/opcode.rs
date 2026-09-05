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

use crate::instruction::declare_instruction_set;
pub use crate::instruction::{Instruction, Operand};

/// Built-in function IDs used with the BUILTIN opcode.
///
/// Re-exported here because built-ins are addressed through the `BUILTIN`
/// opcode's operand; the definitions live in their own module.
pub use crate::builtin;

/// Well-known function block type IDs for intrinsic dispatch.
///
/// Re-exported here because FB types are addressed through the `FB_CALL`
/// opcode's operand; the definitions live in their own module.
pub use crate::fb_type;

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
// 63 of the 64 op-class slots are assigned; 0x3F is the only one left.
// The budget is a hard cap, not a guideline: `encode_opcode` shifts the
// class left by two bits, so a 65th class would wrap into another
// class's opcode bytes. `encode_opcode` asserts the cap, and because
// every opcode constant is derived through it in a const context, an
// over-range class is a compile error rather than a silent collision.
//
// Adding a top-level operation once 0x3F is spent means consolidating a
// family behind sub-opcode dispatch first (see ADR-0033 and its
// amendment).

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
/// Op class: store into an array's storage. Type tag selects the granularity:
/// 0 = STORE_ARRAY (one element at a runtime index), 1 = COPY_REGION (the
/// whole region). Tags 2..3 are free.
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
/// Op class: METHOD_CALL (OOP extension, ADR-0041 Phase 1 static dispatch).
pub const OP_CLASS_METHOD_CALL: u8 = 0x3E;
// 0x3F free (1 op-class slot reserved for future use).

/// Decompose a primary opcode byte into `(op_class, type_tag)`.
#[inline]
pub const fn decode_opcode(op: Opcode) -> (u8, u8) {
    (op >> 2, op & 0x03)
}

/// The largest op class the `[op_class:6][type:2]` encoding can hold.
///
/// The 6-bit field gives 64 slots, 0x00 through 0x3F.
pub const MAX_OP_CLASS: u8 = 0x3F;

/// Compose `(op_class, type_tag)` into a primary opcode byte.
///
/// # Panics
///
/// When `op_class` exceeds [`MAX_OP_CLASS`]. The shift would otherwise
/// drop the high bits and alias another class -- class 64 lands on the
/// same bytes as class 0. Every opcode constant is derived through this
/// function in a const context, so the panic is a compile error for the
/// pull request that adds the 65th op class.
#[inline]
pub const fn encode_opcode(op_class: u8, type_tag: u8) -> Opcode {
    assert!(
        op_class <= MAX_OP_CLASS,
        "op_class does not fit the 6-bit field; consolidate a family behind \
         sub-opcode dispatch instead of claiming a 65th op class"
    );
    (op_class << 2) | (type_tag & 0x03)
}

declare_instruction_set! {
    /// Load a 32-bit integer constant from the constant pool.
    /// Operand: u16 constant pool index (little-endian).
    LOAD_CONST_I32 = (OP_CLASS_LOAD_CONST, T_I32) => [ConstIndex];

    /// Push I32 value 1 (boolean TRUE). Encoded as `LOAD_BOOL` with type tag = 1.
    LOAD_TRUE = (OP_CLASS_LOAD_BOOL, 1) => [];

    /// Push I32 value 0 (boolean FALSE). Encoded as `LOAD_BOOL` with type tag = 0.
    LOAD_FALSE = (OP_CLASS_LOAD_BOOL, 0) => [];

    /// Load a 32-bit integer from the variable table.
    /// Operand: u16 variable index (little-endian).
    LOAD_VAR_I32 = (OP_CLASS_LOAD_VAR, T_I32) => [VarIndex];

    /// Store a 32-bit integer to the variable table.
    /// Operand: u16 variable index (little-endian).
    STORE_VAR_I32 = (OP_CLASS_STORE_VAR, T_I32) => [VarIndex];

    /// Add two 32-bit integers (wrapping).
    /// Pops two values, pushes their sum.
    ADD_I32 = (OP_CLASS_ADD, T_I32) => [];

    /// Subtract two 32-bit integers (wrapping).
    /// Pops two values (b then a), pushes a - b.
    SUB_I32 = (OP_CLASS_SUB, T_I32) => [];

    /// Multiply two 32-bit integers (wrapping).
    /// Pops two values, pushes their product.
    MUL_I32 = (OP_CLASS_MUL, T_I32) => [];

    /// Divide two 32-bit integers (truncating toward zero).
    /// Pops two values (b then a), pushes a / b.
    /// Traps on division by zero.
    DIV_I32 = (OP_CLASS_DIV_S, T_I32) => [];

    /// Modulo (remainder) of two 32-bit integers (truncating toward zero).
    /// Pops two values (b then a), pushes a % b.
    /// Traps on division by zero.
    MOD_I32 = (OP_CLASS_MOD_S, T_I32) => [];

    /// Negate a 32-bit integer (wrapping).
    /// Pops one value, pushes its negation.
    NEG_I32 = (OP_CLASS_NEG, T_I32) => [];

    /// Compare two 32-bit integers for equality.
    /// Pops two values (b then a), pushes 1 if a == b, else 0.
    EQ_I32 = (OP_CLASS_EQ, T_I32) => [];

    /// Compare two 32-bit integers for inequality.
    /// Pops two values (b then a), pushes 1 if a != b, else 0.
    NE_I32 = (OP_CLASS_NE, T_I32) => [];

    /// Compare two signed 32-bit integers (less than).
    /// Pops two values (b then a), pushes 1 if a < b, else 0.
    LT_I32 = (OP_CLASS_LT_S, T_I32) => [];

    /// Compare two signed 32-bit integers (less than or equal).
    /// Pops two values (b then a), pushes 1 if a <= b, else 0.
    LE_I32 = (OP_CLASS_LE_S, T_I32) => [];

    /// Compare two signed 32-bit integers (greater than).
    /// Pops two values (b then a), pushes 1 if a > b, else 0.
    GT_I32 = (OP_CLASS_GT_S, T_I32) => [];

    /// Compare two signed 32-bit integers (greater than or equal).
    /// Pops two values (b then a), pushes 1 if a >= b, else 0.
    GE_I32 = (OP_CLASS_GE_S, T_I32) => [];

    /// Logical AND of two values.
    /// Pops two values (b then a), coerces non-zero to 1, pushes 1 if both are non-zero, else 0.
    BOOL_AND = (OP_CLASS_BOOL_OP, 0) => [];

    /// Logical OR of two values.
    /// Pops two values (b then a), coerces non-zero to 1, pushes 1 if either is non-zero, else 0.
    BOOL_OR = (OP_CLASS_BOOL_OP, 1) => [];

    /// Logical XOR of two values.
    /// Pops two values (b then a), coerces non-zero to 1, pushes 1 if exactly one is non-zero, else 0.
    BOOL_XOR = (OP_CLASS_BOOL_OP, 2) => [];

    /// Logical NOT of a value.
    /// Pops one value, pushes 1 if it is zero, else 0.
    BOOL_NOT = (OP_CLASS_BOOL_OP, 3) => [];

    // --- Bitwise opcodes (32-bit) ---

    /// Bitwise AND of two 32-bit integers.
    /// Pops two values (b then a), pushes a & b.
    BIT_AND_32 = (OP_CLASS_BIT_AND, 0) => [];

    /// Bitwise OR of two 32-bit integers.
    /// Pops two values (b then a), pushes a | b.
    BIT_OR_32 = (OP_CLASS_BIT_OR, 0) => [];

    /// Bitwise XOR of two 32-bit integers.
    /// Pops two values (b then a), pushes a ^ b.
    BIT_XOR_32 = (OP_CLASS_BIT_XOR, 0) => [];

    /// Bitwise NOT of a 32-bit integer.
    /// Pops one value, pushes !a.
    BIT_NOT_32 = (OP_CLASS_BIT_NOT, 0) => [];

    // --- Bitwise opcodes (64-bit) ---

    /// Bitwise AND of two 64-bit integers.
    /// Pops two values (b then a), pushes a & b.
    BIT_AND_64 = (OP_CLASS_BIT_AND, 1) => [];

    /// Bitwise OR of two 64-bit integers.
    /// Pops two values (b then a), pushes a | b.
    BIT_OR_64 = (OP_CLASS_BIT_OR, 1) => [];

    /// Bitwise XOR of two 64-bit integers.
    /// Pops two values (b then a), pushes a ^ b.
    BIT_XOR_64 = (OP_CLASS_BIT_XOR, 1) => [];

    /// Bitwise NOT of a 64-bit integer.
    /// Pops one value, pushes !a.
    BIT_NOT_64 = (OP_CLASS_BIT_NOT, 1) => [];

    /// Unconditional jump. Operand: i16 offset relative to next instruction.
    JMP = (OP_CLASS_JMP, 0) => [JumpOffset];

    /// Jump if top of stack is zero (FALSE). Operand: i16 offset. Pops condition.
    JMP_IF_NOT = (OP_CLASS_JMP_IF_NOT, 0) => [JumpOffset];

    /// Call a built-in standard library function.
    /// Operand: u16 function ID (little-endian).
    /// Stack effect depends on the specific function.
    BUILTIN = (OP_CLASS_BUILTIN, 0) => [BuiltinId];

    /// Call function by index. Pops arguments, executes function body,
    /// pushes return value.
    /// Operand: u16 function_id (little-endian).
    CALL = (OP_CLASS_CALL, 0) => [FunctionId, ParamVarOffset];

    /// Return from function with a value on the stack.
    RET = (OP_CLASS_RET, 0) => [];

    /// Return from the current function (void return).
    RET_VOID = (OP_CLASS_RET_VOID, 0) => [];

    /// Discard the top value from the operand stack.
    POP = (OP_CLASS_STACK_OP, 0) => [];

    /// Duplicate the top value on the operand stack.
    /// Stack effect: [..., a] -> [..., a, a]
    DUP = (OP_CLASS_STACK_OP, 1) => [];

    /// Swap the top two values on the operand stack.
    /// Stack effect: [..., a, b] -> [..., b, a]
    SWAP = (OP_CLASS_STACK_OP, 2) => [];

    // --- Function block opcodes ---

    /// Push FB instance reference from variable table.
    /// Operand: u16 variable index (little-endian).
    FB_LOAD_INSTANCE = (OP_CLASS_FB_LOAD_INSTANCE, 0) => [VarIndex];

    /// Store input parameter on FB instance; keeps fb_ref on stack.
    /// Operand: u8 field index.
    FB_STORE_PARAM = (OP_CLASS_FB_STORE_PARAM, 0) => [FieldIndex];

    /// Load output parameter from FB instance; keeps fb_ref on stack.
    /// Operand: u8 field index.
    FB_LOAD_PARAM = (OP_CLASS_FB_LOAD_PARAM, 0) => [FieldIndex];

    /// Call function block (VM dispatches to intrinsic or bytecode body).
    /// Operand: u16 type_id (little-endian).
    FB_CALL = (OP_CLASS_FB_CALL, 0) => [FbTypeId];

    /// Call a METHOD declared on a function block (OOP extension, ADR-0041
    /// Phase 1 static dispatch). Deliberately simpler than `FB_CALL`: a
    /// method call is always user-defined, never an intrinsic FB type.
    ///
    /// Stack effect: `[..., fb_ref, arg1, .., argN] -> [..., fb_ref (,
    /// return_value)]` — copies the instance's fields into the shared
    /// per-type scratch region (same copy-in `FB_CALL` uses), pops `N`
    /// positional args (read from the resolved function's own parameter
    /// count) into the method's own param slots, and runs the method body.
    /// A method with a return type leaves the value on the stack (ends with
    /// `RET`); a void method does not (ends with `RET_VOID`). Either way,
    /// `fb_ref` is left underneath for the caller to discard, matching
    /// `FB_CALL`.
    ///
    /// Operands (u16 fields little-endian):
    /// - u16 `function_id`: the method's compiled function
    /// - u16 `field_var_off`: start of the owning FB type's field scratch region
    /// - u8 `num_fields`: number of fields to copy in/out (matches the u8
    ///   width already used for `num_fields` throughout the FB-instance
    ///   machinery, e.g. `FbCallReturn::num_fields`)
    /// - u16 `param_var_off`: start of the method's own param/local scratch region
    METHOD_CALL = (OP_CLASS_METHOD_CALL, 0) => [FunctionId, FieldVarOffset, NumFields, ParamVarOffset];

    // --- String opcodes ---

    /// Load a STRING literal from the constant pool into a temporary buffer.
    /// Operand: u16 constant pool index (little-endian).
    /// Pushes the temp buf_idx onto the stack.
    LOAD_CONST_STR = (OP_CLASS_LOAD_CONST_STR, 0) => [ConstIndex];

    /// Initialize a STRING variable in the data region.
    /// Operands: data_offset: u32, max_length: u16.
    /// Sets max_length and cur_length=0 at the given data_offset.
    STR_INIT = (OP_CLASS_STR_INIT, 0) => [DataOffset, MaxLength, CharWidth];

    /// Copy STRING from data region into a temp buffer; push temp buf_idx.
    /// Operand: data_offset: u32.
    STR_LOAD_VAR = (OP_CLASS_STR_LOAD_VAR, 0) => [DataOffset];

    /// Copy temp buffer contents into STRING variable at data_offset.
    /// Operand: data_offset: u32. Pops buf_idx from stack.
    STR_STORE_VAR = (OP_CLASS_STR_STORE_VAR, 0) => [DataOffset];

    /// Read the current length of a STRING variable from the data region.
    /// Operand: data_offset: u32.
    /// Pushes the cur_length as an i32 onto the stack.
    LEN_STR = (OP_CLASS_LEN_STR, 0) => [DataOffset];

    /// Find the first occurrence of IN2 within IN1.
    /// Operands: in1_data_offset: u32, in2_data_offset: u32.
    /// Pushes the 1-based position as i32 (0 if not found).
    FIND_STR = (OP_CLASS_FIND_STR, 0) => [DataOffset, DataOffset];

    /// Replace L characters starting at position P in IN1 with IN2.
    /// Operands: in1_data_offset: u32, in2_data_offset: u32.
    /// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
    REPLACE_STR = (OP_CLASS_REPLACE_STR, 0) => [DataOffset, DataOffset];

    /// Insert IN2 into IN1 after position P.
    /// Operands: in1_data_offset: u32, in2_data_offset: u32.
    /// Pops P (i32) from stack. Pushes buf_idx (i32).
    INSERT_STR = (OP_CLASS_INSERT_STR, 0) => [DataOffset, DataOffset];

    /// Delete L characters from IN1 starting at position P.
    /// Operand: in1_data_offset: u32.
    /// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
    DELETE_STR = (OP_CLASS_DELETE_STR, 0) => [DataOffset];

    /// Return the leftmost L characters of IN.
    /// Operand: in_data_offset: u32.
    /// Pops L (i32) from stack. Pushes buf_idx (i32).
    LEFT_STR = (OP_CLASS_LEFT_STR, 0) => [DataOffset];

    /// Return the rightmost L characters of IN.
    /// Operand: in_data_offset: u32.
    /// Pops L (i32) from stack. Pushes buf_idx (i32).
    RIGHT_STR = (OP_CLASS_RIGHT_STR, 0) => [DataOffset];

    /// Return L characters from IN starting at position P.
    /// Operand: in_data_offset: u32.
    /// Pops P (i32) then L (i32) from stack. Pushes buf_idx (i32).
    MID_STR = (OP_CLASS_MID_STR, 0) => [DataOffset];

    /// Concatenate IN1 and IN2.
    /// Operands: in1_data_offset: u32, in2_data_offset: u32.
    /// Pushes buf_idx (i32).
    CONCAT_STR = (OP_CLASS_CONCAT_STR, 0) => [DataOffset, DataOffset];

    // --- String array opcodes ---

    /// Initialize all string headers in an array of strings.
    /// Operand 1: u16 variable table index (base data_offset).
    /// Operand 2: u16 array descriptor index.
    /// Uses element_extra from the descriptor as max_string_length.
    /// Stack effect: none.
    STR_INIT_ARRAY = (OP_CLASS_STR_INIT_ARRAY, 0) => [VarIndex, ArrayDescIndex];

    /// Load a string from an array element into a temp buffer.
    /// Operand 1: u16 variable table index (base data_offset).
    /// Operand 2: u16 array descriptor index.
    /// Pops flat_index, pushes buf_idx. Net stack: 0.
    STR_LOAD_ARRAY_ELEM = (OP_CLASS_STR_LOAD_ARRAY_ELEM, 0) => [VarIndex, ArrayDescIndex];

    /// Store a temp buffer into an array element's string slot.
    /// Operand 1: u16 variable table index (base data_offset).
    /// Operand 2: u16 array descriptor index.
    /// Pops flat_index, then pops buf_idx. Net stack: -2.
    STR_STORE_ARRAY_ELEM = (OP_CLASS_STR_STORE_ARRAY_ELEM, 0) => [VarIndex, ArrayDescIndex];

    // --- Array opcodes ---

    /// Load a value from an array element.
    /// Operand 1: u16 variable table index (little-endian).
    /// Operand 2: u16 array descriptor index (little-endian).
    /// Pops 1 (flat index), pushes 1 (element value). Net stack: 0.
    LOAD_ARRAY = (OP_CLASS_LOAD_ARRAY, 0) => [VarIndex, ArrayDescIndex];

    /// Store a value to an array element.
    /// Operand 1: u16 variable table index (little-endian).
    /// Operand 2: u16 array descriptor index (little-endian).
    /// Pops 2 (value, flat index). Net stack: -2.
    STORE_ARRAY = (OP_CLASS_STORE_ARRAY, 0) => [VarIndex, ArrayDescIndex];

    /// Load a value from an array element through a reference (double indirection).
    /// Operand 1: u16 reference variable index (little-endian). The slot holds the
    ///            target array's variable index.
    /// Operand 2: u16 array descriptor index (little-endian).
    /// Pops 1 (flat index), pushes 1 (element value). Net stack: 0.
    LOAD_ARRAY_DEREF = (OP_CLASS_LOAD_ARRAY_DEREF, 0) => [RefIndex, ArrayDescIndex];

    /// Store a value to an array element through a reference (double indirection).
    /// Operand 1: u16 reference variable index (little-endian). The slot holds the
    ///            target array's variable index.
    /// Operand 2: u16 array descriptor index (little-endian).
    /// Pops 2 (value, flat index). Net stack: -2.
    STORE_ARRAY_DEREF = (OP_CLASS_STORE_ARRAY_DEREF, 0) => [RefIndex, ArrayDescIndex];

    /// Copy a whole aggregate (array or structure) within the data region.
    ///
    /// Operand 1: u16 destination variable index (little-endian). The slot holds
    ///            the destination's data-region byte offset.
    /// Operand 2: u16 destination array descriptor index (little-endian).
    /// Operand 3: u16 source array descriptor index (little-endian).
    /// Pops 1 (source data-region byte offset). Net stack: -1.
    ///
    /// The copy length is *not* an operand: the VM derives it from both
    /// descriptors and traps [`crate::opcode`] callers with `RegionSizeMismatch`
    /// if the two disagree. Carrying a length immediate instead would let a
    /// codegen defect over-copy into a neighbouring variable, which is precisely
    /// the class of bug this opcode exists to prevent.
    ///
    /// The destination is named by variable index so it is scope-checked; the
    /// source arrives as a stack offset so that a struct-returning function call,
    /// which leaves its `data_offset` on the stack and has no variable index in
    /// the caller's scope, uses the same instruction.
    ///
    /// Overlapping regions are well defined (the VM uses `copy_within`), so
    /// `x := x` is a no-op rather than corruption.
    ///
    /// Encoded as a type-tag variant of `STORE_ARRAY` — the same op class, one
    /// granularity coarser — rather than as an op class of its own. Op classes
    /// exist to keep the dispatch table small, not to name operations, and
    /// spending one on a single instruction would have consumed the last free
    /// slot.
    COPY_REGION = (OP_CLASS_STORE_ARRAY, 1) => [VarIndex, ArrayDescIndex, ArrayDescIndex]
        note "size from descriptors; source offset from stack";

    // --- Truncation opcodes ---

    /// Truncate i32 to i8 range, then sign-extend back to i32.
    /// `(v as i8) as i32` — wraps to -128..127.
    TRUNC_I8 = (OP_CLASS_TRUNC, 0) => [];

    /// Truncate i32 to u8 range, then zero-extend back to i32.
    /// `(v as u8) as i32` — wraps to 0..255.
    TRUNC_U8 = (OP_CLASS_TRUNC, 1) => [];

    /// Truncate i32 to i16 range, then sign-extend back to i32.
    /// `(v as i16) as i32` — wraps to -32768..32767.
    TRUNC_I16 = (OP_CLASS_TRUNC, 2) => [];

    /// Truncate i32 to u16 range, then zero-extend back to i32.
    /// `(v as u16) as i32` — wraps to 0..65535.
    TRUNC_U16 = (OP_CLASS_TRUNC, 3) => [];

    // --- 64-bit load/store opcodes ---

    /// Load a 64-bit integer constant from the constant pool.
    /// Operand: u16 constant pool index (little-endian).
    LOAD_CONST_I64 = (OP_CLASS_LOAD_CONST, T_I64) => [ConstIndex];

    /// Load a 32-bit float constant from the constant pool.
    /// Operand: u16 constant pool index (little-endian).
    LOAD_CONST_F32 = (OP_CLASS_LOAD_CONST, T_F32) => [ConstIndex];

    /// Load a 64-bit float constant from the constant pool.
    /// Operand: u16 constant pool index (little-endian).
    LOAD_CONST_F64 = (OP_CLASS_LOAD_CONST, T_F64) => [ConstIndex];

    /// Load a 64-bit integer from the variable table.
    /// Operand: u16 variable index (little-endian).
    LOAD_VAR_I64 = (OP_CLASS_LOAD_VAR, T_I64) => [VarIndex];

    /// Load a 32-bit float from the variable table.
    /// Operand: u16 variable index (little-endian).
    LOAD_VAR_F32 = (OP_CLASS_LOAD_VAR, T_F32) => [VarIndex];

    /// Load a 64-bit float from the variable table.
    /// Operand: u16 variable index (little-endian).
    LOAD_VAR_F64 = (OP_CLASS_LOAD_VAR, T_F64) => [VarIndex];

    /// Indirect load: pops a reference (variable index) from the stack,
    /// loads the referenced variable's value, and pushes it.
    /// No operand. Stack: [..., ref] → [..., value].
    LOAD_INDIRECT = (OP_CLASS_LOAD_INDIRECT, 0) => [];

    /// Indirect store: pops a value and a reference (variable index) from the stack,
    /// stores the value into the referenced variable.
    /// No operand. Stack: [..., value, ref] → [...].
    STORE_INDIRECT = (OP_CLASS_STORE_INDIRECT, 0) => [];

    /// Store a 64-bit integer to the variable table.
    /// Operand: u16 variable index (little-endian).
    STORE_VAR_I64 = (OP_CLASS_STORE_VAR, T_I64) => [VarIndex];

    /// Store a 32-bit float to the variable table.
    /// Operand: u16 variable index (little-endian).
    STORE_VAR_F32 = (OP_CLASS_STORE_VAR, T_F32) => [VarIndex];

    /// Store a 64-bit float to the variable table.
    /// Operand: u16 variable index (little-endian).
    STORE_VAR_F64 = (OP_CLASS_STORE_VAR, T_F64) => [VarIndex];

    // --- 64-bit arithmetic opcodes ---

    /// Add two 64-bit integers (wrapping).
    /// Pops two values (b then a), pushes a.wrapping_add(b).
    ADD_I64 = (OP_CLASS_ADD, T_I64) => [];

    /// Subtract two 64-bit integers (wrapping).
    /// Pops two values (b then a), pushes a.wrapping_sub(b).
    SUB_I64 = (OP_CLASS_SUB, T_I64) => [];

    /// Multiply two 64-bit integers (wrapping).
    /// Pops two values (b then a), pushes a.wrapping_mul(b).
    MUL_I64 = (OP_CLASS_MUL, T_I64) => [];

    /// Divide two signed 64-bit integers (truncating toward zero).
    /// Pops two values (b then a), pushes a / b. Traps on division by zero.
    DIV_I64 = (OP_CLASS_DIV_S, T_I64) => [];

    /// Modulo (remainder) of two signed 64-bit integers.
    /// Pops two values (b then a), pushes a % b. Traps on division by zero.
    MOD_I64 = (OP_CLASS_MOD_S, T_I64) => [];

    /// Negate a 64-bit integer (wrapping).
    /// Pops one value, pushes its negation.
    NEG_I64 = (OP_CLASS_NEG, T_I64) => [];

    // --- Unsigned 32-bit division opcodes ---

    /// Divide two unsigned 32-bit integers.
    /// Pops two i32 values (b then a), reinterprets as u32, pushes (a/b) as i32.
    /// Traps on division by zero.
    DIV_U32 = (OP_CLASS_DIV_U, T_I32) => [];

    /// Modulo (remainder) of two unsigned 32-bit integers.
    /// Pops two i32 values (b then a), reinterprets as u32, pushes (a%b) as i32.
    /// Traps on division by zero.
    MOD_U32 = (OP_CLASS_MOD_U, T_I32) => [];

    /// Divide two unsigned 64-bit integers.
    /// Pops two i64 values (b then a), reinterprets as u64, pushes (a/b) as i64.
    /// Traps on division by zero.
    DIV_U64 = (OP_CLASS_DIV_U, T_I64) => [];

    /// Modulo (remainder) of two unsigned 64-bit integers.
    /// Pops two i64 values (b then a), reinterprets as u64, pushes (a%b) as i64.
    /// Traps on division by zero.
    MOD_U64 = (OP_CLASS_MOD_U, T_I64) => [];

    // --- 32-bit float arithmetic opcodes ---

    /// Add two 32-bit floats.
    /// Pops two values (b then a), pushes a + b.
    ADD_F32 = (OP_CLASS_ADD, T_F32) => [];

    /// Subtract two 32-bit floats.
    /// Pops two values (b then a), pushes a - b.
    SUB_F32 = (OP_CLASS_SUB, T_F32) => [];

    /// Multiply two 32-bit floats.
    /// Pops two values (b then a), pushes a * b.
    MUL_F32 = (OP_CLASS_MUL, T_F32) => [];

    /// Divide two 32-bit floats.
    /// Pops two values (b then a), pushes a / b.
    /// IEEE 754: produces ±Inf or NaN on division by zero.
    DIV_F32 = (OP_CLASS_DIV_S, T_F32) => [];

    /// Negate a 32-bit float.
    /// Pops one value, pushes its negation.
    NEG_F32 = (OP_CLASS_NEG, T_F32) => [];

    // --- 64-bit float arithmetic opcodes ---

    /// Add two 64-bit floats.
    /// Pops two values (b then a), pushes a + b.
    ADD_F64 = (OP_CLASS_ADD, T_F64) => [];

    /// Subtract two 64-bit floats.
    /// Pops two values (b then a), pushes a - b.
    SUB_F64 = (OP_CLASS_SUB, T_F64) => [];

    /// Multiply two 64-bit floats.
    /// Pops two values (b then a), pushes a * b.
    MUL_F64 = (OP_CLASS_MUL, T_F64) => [];

    /// Divide two 64-bit floats.
    /// Pops two values (b then a), pushes a / b.
    /// IEEE 754: produces ±Inf or NaN on division by zero.
    DIV_F64 = (OP_CLASS_DIV_S, T_F64) => [];

    /// Negate a 64-bit float.
    /// Pops one value, pushes its negation.
    NEG_F64 = (OP_CLASS_NEG, T_F64) => [];

    // --- 64-bit comparison opcodes ---

    /// Compare two 64-bit integers for equality.
    /// Pops two values (b then a), pushes 1 if a == b, else 0.
    EQ_I64 = (OP_CLASS_EQ, T_I64) => [];

    /// Compare two 64-bit integers for inequality.
    /// Pops two values (b then a), pushes 1 if a != b, else 0.
    NE_I64 = (OP_CLASS_NE, T_I64) => [];

    /// Compare two signed 64-bit integers (less than).
    /// Pops two values (b then a), pushes 1 if a < b, else 0.
    LT_I64 = (OP_CLASS_LT_S, T_I64) => [];

    /// Compare two signed 64-bit integers (less than or equal).
    /// Pops two values (b then a), pushes 1 if a <= b, else 0.
    LE_I64 = (OP_CLASS_LE_S, T_I64) => [];

    /// Compare two signed 64-bit integers (greater than).
    /// Pops two values (b then a), pushes 1 if a > b, else 0.
    GT_I64 = (OP_CLASS_GT_S, T_I64) => [];

    /// Compare two signed 64-bit integers (greater than or equal).
    /// Pops two values (b then a), pushes 1 if a >= b, else 0.
    GE_I64 = (OP_CLASS_GE_S, T_I64) => [];

    // --- Unsigned comparison opcodes ---

    /// Compare two unsigned 32-bit integers (less than).
    /// Pops two i32 values (b then a), pushes 1 if (a as u32) < (b as u32), else 0.
    LT_U32 = (OP_CLASS_LT_U, T_I32) => [];

    /// Compare two unsigned 32-bit integers (less than or equal).
    /// Pops two i32 values (b then a), pushes 1 if (a as u32) <= (b as u32), else 0.
    LE_U32 = (OP_CLASS_LE_U, T_I32) => [];

    /// Compare two unsigned 32-bit integers (greater than).
    /// Pops two i32 values (b then a), pushes 1 if (a as u32) > (b as u32), else 0.
    GT_U32 = (OP_CLASS_GT_U, T_I32) => [];

    /// Compare two unsigned 32-bit integers (greater than or equal).
    /// Pops two i32 values (b then a), pushes 1 if (a as u32) >= (b as u32), else 0.
    GE_U32 = (OP_CLASS_GE_U, T_I32) => [];

    /// Compare two unsigned 64-bit integers (less than).
    /// Pops two i64 values (b then a), pushes 1 if (a as u64) < (b as u64), else 0.
    LT_U64 = (OP_CLASS_LT_U, T_I64) => [];

    /// Compare two unsigned 64-bit integers (less than or equal).
    /// Pops two i64 values (b then a), pushes 1 if (a as u64) <= (b as u64), else 0.
    LE_U64 = (OP_CLASS_LE_U, T_I64) => [];

    /// Compare two unsigned 64-bit integers (greater than).
    /// Pops two i64 values (b then a), pushes 1 if (a as u64) > (b as u64), else 0.
    GT_U64 = (OP_CLASS_GT_U, T_I64) => [];

    /// Compare two unsigned 64-bit integers (greater than or equal).
    /// Pops two i64 values (b then a), pushes 1 if (a as u64) >= (b as u64), else 0.
    GE_U64 = (OP_CLASS_GE_U, T_I64) => [];

    // --- 32-bit float comparison opcodes ---

    /// Compare two 32-bit floats for equality.
    /// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
    EQ_F32 = (OP_CLASS_EQ, T_F32) => [];

    /// Compare two 32-bit floats for inequality.
    /// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
    NE_F32 = (OP_CLASS_NE, T_F32) => [];

    /// Compare two 32-bit floats (less than).
    /// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
    LT_F32 = (OP_CLASS_LT_S, T_F32) => [];

    /// Compare two 32-bit floats (less than or equal).
    /// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
    LE_F32 = (OP_CLASS_LE_S, T_F32) => [];

    /// Compare two 32-bit floats (greater than).
    /// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
    GT_F32 = (OP_CLASS_GT_S, T_F32) => [];

    /// Compare two 32-bit floats (greater than or equal).
    /// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
    GE_F32 = (OP_CLASS_GE_S, T_F32) => [];

    // --- 64-bit float comparison opcodes ---

    /// Compare two 64-bit floats for equality.
    /// Pops two values (b then a), pushes 1 if a == b, else 0 (as i32).
    EQ_F64 = (OP_CLASS_EQ, T_F64) => [];

    /// Compare two 64-bit floats for inequality.
    /// Pops two values (b then a), pushes 1 if a != b, else 0 (as i32).
    NE_F64 = (OP_CLASS_NE, T_F64) => [];

    /// Compare two 64-bit floats (less than).
    /// Pops two values (b then a), pushes 1 if a < b, else 0 (as i32).
    LT_F64 = (OP_CLASS_LT_S, T_F64) => [];

    /// Compare two 64-bit floats (less than or equal).
    /// Pops two values (b then a), pushes 1 if a <= b, else 0 (as i32).
    LE_F64 = (OP_CLASS_LE_S, T_F64) => [];

    /// Compare two 64-bit floats (greater than).
    /// Pops two values (b then a), pushes 1 if a > b, else 0 (as i32).
    GT_F64 = (OP_CLASS_GT_S, T_F64) => [];

    /// Compare two 64-bit floats (greater than or equal).
    /// Pops two values (b then a), pushes 1 if a >= b, else 0 (as i32).
    GE_F64 = (OP_CLASS_GE_S, T_F64) => [];

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
    CMP_BR_I32 = (OP_CLASS_CMP_BR, T_I32) => [CmpOp, VarIndex, ConstIndex, JumpOffset];

    /// Fused compare-and-branch on 64-bit signed integers.
    /// See `CMP_BR_I32` for operand layout and semantics.
    CMP_BR_I64 = (OP_CLASS_CMP_BR, T_I64) => [CmpOp, VarIndex, ConstIndex, JumpOffset];
}

/// Comparison-operator codes used as the first operand of `CMP_BR_*`.
///
/// Re-exported here because a comparison operator is addressed through the
/// `CMP_BR_*` operand; the definitions live in their own module.
pub use crate::cmp_op;

/// Returns the total byte size of the instruction starting with `op`.
///
/// This is the single source of truth for instruction sizes, shared by both
/// the emitter and the optimizer. Keeping one function prevents the two from
/// diverging and producing misaligned instruction boundaries.
pub fn instruction_size(op: Opcode) -> usize {
    // Unassigned bytes advance by 1 so disassembly cannot get stuck.
    instruction_size_opt(op).unwrap_or(1)
}

/// Returns `true` iff `op` is an assigned opcode with a defined encoding.
///
/// Derived from [`instruction_size_opt`] so it stays exhaustive automatically:
/// a new opcode added there is immediately recognized here and by the
/// wire-format completeness guard in `codegen/tests/it/wire_format.rs`, which
/// fails until the new opcode's byte value is pinned.
pub fn is_assigned(op: Opcode) -> bool {
    instruction_size_opt(op).is_some()
}

/// Byte size of the instruction starting with `op`, or `None` for an
/// unassigned byte: the opcode byte plus the width of each of its operands.
fn instruction_size_opt(op: Opcode) -> Option<usize> {
    let instruction = Instruction::decode(op)?;
    Some(
        1 + instruction
            .operands
            .iter()
            .map(|o| o.width())
            .sum::<usize>(),
    )
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
    fn decode_when_unassigned_byte_then_returns_none() {
        assert_eq!(Instruction::decode(0xFE), None);
    }

    #[test]
    fn decode_when_assigned_opcode_then_mnemonic_is_its_name() {
        assert_eq!(Instruction::decode(ADD_I32).unwrap().mnemonic, "ADD_I32");
        assert_eq!(
            Instruction::decode(STORE_VAR_F64).unwrap().mnemonic,
            "STORE_VAR_F64"
        );
    }

    #[test]
    fn decode_when_opcode_assigned_then_every_one_decodes() {
        // The table gives an opcode its name and operands together, so
        // `is_assigned` and `decode` cannot disagree about which bytes are
        // opcodes.
        for op in 0..=u8::MAX {
            assert_eq!(
                is_assigned(op),
                Instruction::decode(op).is_some(),
                "opcode 0x{op:02X}"
            );
        }
    }

    #[test]
    fn instruction_size_when_opcode_assigned_then_matches_wire_format() {
        // One opcode per instruction shape, pinned to the byte counts the
        // wire format has always used. These are asserted independently of
        // the table so a mistyped operand layout is caught here.
        assert_eq!(instruction_size(ADD_I32), 1);
        assert_eq!(instruction_size(FB_STORE_PARAM), 2);
        assert_eq!(instruction_size(LOAD_VAR_I32), 3);
        assert_eq!(instruction_size(CALL), 5);
        assert_eq!(instruction_size(LEN_STR), 5);
        assert_eq!(instruction_size(STR_INIT), 8);
        assert_eq!(instruction_size(CMP_BR_I32), 8);
        assert_eq!(instruction_size(METHOD_CALL), 8);
        assert_eq!(instruction_size(FIND_STR), 9);
    }

    #[test]
    fn decode_when_no_operand_opcode_then_operands_are_empty() {
        assert_eq!(Instruction::decode(ADD_I32).unwrap().operands, &[]);
    }

    #[test]
    fn decode_when_multi_operand_opcode_then_operands_are_in_order() {
        assert_eq!(
            Instruction::decode(CMP_BR_I32).unwrap().operands,
            &[
                Operand::CmpOp,
                Operand::VarIndex,
                Operand::ConstIndex,
                Operand::JumpOffset
            ]
        );
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

    #[test]
    fn arg_count_when_unnamed_arithmetic_builtins_then_counts_match_dispatch() {
        assert_eq!(builtin::arg_count(builtin::TRUNC_F64), 1);
        assert_eq!(builtin::arg_count(builtin::MOD_F64), 2);
        assert_eq!(builtin::arg_count(builtin::TRUNC_F32), 1);
        assert_eq!(builtin::arg_count(builtin::MOD_F32), 2);
    }
}
