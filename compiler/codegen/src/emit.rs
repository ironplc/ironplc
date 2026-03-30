//! Low-level bytecode emitter.
//!
//! Provides a builder that appends opcodes and operands to a byte buffer.

use ironplc_container::opcode;
use ironplc_container::VarIndex;

/// An opaque forward reference to a bytecode position, used for jump targets.
#[derive(Clone, Copy)]
pub struct Label(usize);

/// A pending jump that needs to be patched once the target label is bound.
struct PendingPatch {
    /// Position of the i16 operand in the bytecode buffer.
    patch_offset: usize,
    /// The label this jump targets.
    label: Label,
}

/// Accumulates bytecode instructions.
pub struct Emitter {
    bytecode: Vec<u8>,
    max_stack_depth: u16,
    current_stack_depth: u16,
    /// Bound positions for each label (None if not yet bound).
    labels: Vec<Option<usize>>,
    /// Jump operands that need backpatching.
    patches: Vec<PendingPatch>,
}

/// Emit a no-operand instruction that pushes one value.
macro_rules! emit_push_op {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self) {
            self.bytecode.push($opcode);
            self.push_stack(1);
        }
    };
}

/// Emit an instruction with a u16 operand that pushes one value.
macro_rules! emit_load_u16 {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self, index: u16) {
            self.bytecode.push($opcode);
            self.bytecode.extend_from_slice(&index.to_le_bytes());
            self.push_stack(1);
        }
    };
}

/// Emit an instruction with a VarIndex operand that pushes one value.
macro_rules! emit_load_var_index {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self, index: VarIndex) {
            self.bytecode.push($opcode);
            self.bytecode.extend_from_slice(&index.to_le_bytes());
            self.push_stack(1);
        }
    };
}

/// Emit an instruction with a VarIndex operand that pops one value.
macro_rules! emit_store_var_index {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self, index: VarIndex) {
            self.bytecode.push($opcode);
            self.bytecode.extend_from_slice(&index.to_le_bytes());
            self.pop_stack(1);
        }
    };
}

/// Emit a no-operand binary op (pops two, pushes one = net pop 1).
macro_rules! emit_binop {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self) {
            self.bytecode.push($opcode);
            self.pop_stack(1);
        }
    };
}

/// Emit a no-operand unary op (pops one, pushes one = no stack change).
macro_rules! emit_unaryop {
    ($name:ident, $opcode:expr) => {
        pub fn $name(&mut self) {
            self.bytecode.push($opcode);
        }
    };
}

impl Emitter {
    pub fn new() -> Self {
        Emitter {
            bytecode: Vec::new(),
            max_stack_depth: 0,
            current_stack_depth: 0,
            labels: Vec::new(),
            patches: Vec::new(),
        }
    }

    // --- Push ops (no operand, push 1) ---
    emit_push_op!(emit_load_true, opcode::LOAD_TRUE);
    emit_push_op!(emit_load_false, opcode::LOAD_FALSE);

    // --- Load ops (u16 operand, push 1) ---
    emit_load_u16!(emit_load_const_i32, opcode::LOAD_CONST_I32);
    emit_load_u16!(emit_load_const_i64, opcode::LOAD_CONST_I64);
    emit_load_u16!(emit_load_const_f32, opcode::LOAD_CONST_F32);
    emit_load_u16!(emit_load_const_f64, opcode::LOAD_CONST_F64);
    emit_load_var_index!(emit_load_var_i32, opcode::LOAD_VAR_I32);
    emit_load_var_index!(emit_load_var_i64, opcode::LOAD_VAR_I64);
    emit_load_var_index!(emit_load_var_f32, opcode::LOAD_VAR_F32);
    emit_load_var_index!(emit_load_var_f64, opcode::LOAD_VAR_F64);

    // --- Store ops (VarIndex operand, pop 1) ---
    emit_store_var_index!(emit_store_var_i32, opcode::STORE_VAR_I32);
    emit_store_var_index!(emit_store_var_i64, opcode::STORE_VAR_I64);
    emit_store_var_index!(emit_store_var_f32, opcode::STORE_VAR_F32);
    emit_store_var_index!(emit_store_var_f64, opcode::STORE_VAR_F64);

    // --- Binary ops (pops 2, pushes 1 = net pop 1) ---
    emit_binop!(emit_add_i32, opcode::ADD_I32);
    emit_binop!(emit_sub_i32, opcode::SUB_I32);
    emit_binop!(emit_mul_i32, opcode::MUL_I32);
    emit_binop!(emit_div_i32, opcode::DIV_I32);
    emit_binop!(emit_mod_i32, opcode::MOD_I32);
    emit_binop!(emit_add_i64, opcode::ADD_I64);
    emit_binop!(emit_sub_i64, opcode::SUB_I64);
    emit_binop!(emit_mul_i64, opcode::MUL_I64);
    emit_binop!(emit_div_i64, opcode::DIV_I64);
    emit_binop!(emit_mod_i64, opcode::MOD_I64);
    emit_binop!(emit_div_u32, opcode::DIV_U32);
    emit_binop!(emit_mod_u32, opcode::MOD_U32);
    emit_binop!(emit_div_u64, opcode::DIV_U64);
    emit_binop!(emit_mod_u64, opcode::MOD_U64);
    emit_binop!(emit_add_f32, opcode::ADD_F32);
    emit_binop!(emit_sub_f32, opcode::SUB_F32);
    emit_binop!(emit_mul_f32, opcode::MUL_F32);
    emit_binop!(emit_div_f32, opcode::DIV_F32);
    emit_binop!(emit_add_f64, opcode::ADD_F64);
    emit_binop!(emit_sub_f64, opcode::SUB_F64);
    emit_binop!(emit_mul_f64, opcode::MUL_F64);
    emit_binop!(emit_div_f64, opcode::DIV_F64);
    emit_binop!(emit_eq_i32, opcode::EQ_I32);
    emit_binop!(emit_ne_i32, opcode::NE_I32);
    emit_binop!(emit_lt_i32, opcode::LT_I32);
    emit_binop!(emit_le_i32, opcode::LE_I32);
    emit_binop!(emit_gt_i32, opcode::GT_I32);
    emit_binop!(emit_ge_i32, opcode::GE_I32);
    emit_binop!(emit_eq_i64, opcode::EQ_I64);
    emit_binop!(emit_ne_i64, opcode::NE_I64);
    emit_binop!(emit_lt_i64, opcode::LT_I64);
    emit_binop!(emit_le_i64, opcode::LE_I64);
    emit_binop!(emit_gt_i64, opcode::GT_I64);
    emit_binop!(emit_ge_i64, opcode::GE_I64);
    emit_binop!(emit_lt_u32, opcode::LT_U32);
    emit_binop!(emit_le_u32, opcode::LE_U32);
    emit_binop!(emit_gt_u32, opcode::GT_U32);
    emit_binop!(emit_ge_u32, opcode::GE_U32);
    emit_binop!(emit_lt_u64, opcode::LT_U64);
    emit_binop!(emit_le_u64, opcode::LE_U64);
    emit_binop!(emit_gt_u64, opcode::GT_U64);
    emit_binop!(emit_ge_u64, opcode::GE_U64);
    emit_binop!(emit_eq_f32, opcode::EQ_F32);
    emit_binop!(emit_ne_f32, opcode::NE_F32);
    emit_binop!(emit_lt_f32, opcode::LT_F32);
    emit_binop!(emit_le_f32, opcode::LE_F32);
    emit_binop!(emit_gt_f32, opcode::GT_F32);
    emit_binop!(emit_ge_f32, opcode::GE_F32);
    emit_binop!(emit_eq_f64, opcode::EQ_F64);
    emit_binop!(emit_ne_f64, opcode::NE_F64);
    emit_binop!(emit_lt_f64, opcode::LT_F64);
    emit_binop!(emit_le_f64, opcode::LE_F64);
    emit_binop!(emit_gt_f64, opcode::GT_F64);
    emit_binop!(emit_ge_f64, opcode::GE_F64);
    emit_binop!(emit_bool_and, opcode::BOOL_AND);
    emit_binop!(emit_bool_or, opcode::BOOL_OR);
    emit_binop!(emit_bool_xor, opcode::BOOL_XOR);
    emit_binop!(emit_bit_and_32, opcode::BIT_AND_32);
    emit_binop!(emit_bit_or_32, opcode::BIT_OR_32);
    emit_binop!(emit_bit_xor_32, opcode::BIT_XOR_32);
    emit_binop!(emit_bit_and_64, opcode::BIT_AND_64);
    emit_binop!(emit_bit_or_64, opcode::BIT_OR_64);
    emit_binop!(emit_bit_xor_64, opcode::BIT_XOR_64);

    // --- Unary ops (pops 1, pushes 1 = no stack change) ---
    emit_unaryop!(emit_neg_i32, opcode::NEG_I32);
    emit_unaryop!(emit_neg_i64, opcode::NEG_I64);
    emit_unaryop!(emit_neg_f32, opcode::NEG_F32);
    emit_unaryop!(emit_neg_f64, opcode::NEG_F64);
    emit_unaryop!(emit_bool_not, opcode::BOOL_NOT);
    emit_unaryop!(emit_bit_not_32, opcode::BIT_NOT_32);
    emit_unaryop!(emit_bit_not_64, opcode::BIT_NOT_64);
    emit_unaryop!(emit_trunc_i8, opcode::TRUNC_I8);
    emit_unaryop!(emit_trunc_u8, opcode::TRUNC_U8);
    emit_unaryop!(emit_trunc_i16, opcode::TRUNC_I16);
    emit_unaryop!(emit_trunc_u16, opcode::TRUNC_U16);

    // --- Reference (indirect) ops ---
    emit_unaryop!(emit_load_indirect, opcode::LOAD_INDIRECT);

    /// Emits STORE_INDIRECT.
    /// Pops 2 (value and reference). Net: -2.
    pub fn emit_store_indirect(&mut self) {
        self.bytecode.push(opcode::STORE_INDIRECT);
        self.pop_stack(2);
    }

    /// Emits LOAD_ARRAY with var_index and desc_index operands.
    /// Pops 1 (flat index already on stack), pushes 1 (element value). Net: 0.
    #[allow(dead_code)]
    pub fn emit_load_array(&mut self, var_index: VarIndex, desc_index: u16) {
        self.bytecode.push(opcode::LOAD_ARRAY);
        self.bytecode.extend_from_slice(&var_index.to_le_bytes());
        self.bytecode.extend_from_slice(&desc_index.to_le_bytes());
        // Pop index, push value = no net change
        self.pop_stack(1);
        self.push_stack(1);
    }

    /// Emits STORE_ARRAY with var_index and desc_index operands.
    /// Pops 2 (value and flat index). Net: -2.
    #[allow(dead_code)]
    pub fn emit_store_array(&mut self, var_index: VarIndex, desc_index: u16) {
        self.bytecode.push(opcode::STORE_ARRAY);
        self.bytecode.extend_from_slice(&var_index.to_le_bytes());
        self.bytecode.extend_from_slice(&desc_index.to_le_bytes());
        self.pop_stack(2);
    }

    /// Emits LOAD_ARRAY_DEREF with ref_var_index and desc_index operands.
    /// Pops 1 (flat index), pushes 1 (element value). Net: 0.
    pub fn emit_load_array_deref(&mut self, ref_var_index: VarIndex, desc_index: u16) {
        self.bytecode.push(opcode::LOAD_ARRAY_DEREF);
        self.bytecode
            .extend_from_slice(&ref_var_index.to_le_bytes());
        self.bytecode.extend_from_slice(&desc_index.to_le_bytes());
        self.pop_stack(1);
        self.push_stack(1);
    }

    /// Emits STORE_ARRAY_DEREF with ref_var_index and desc_index operands.
    /// Pops 2 (value and flat index). Net: -2.
    pub fn emit_store_array_deref(&mut self, ref_var_index: VarIndex, desc_index: u16) {
        self.bytecode.push(opcode::STORE_ARRAY_DEREF);
        self.bytecode
            .extend_from_slice(&ref_var_index.to_le_bytes());
        self.bytecode.extend_from_slice(&desc_index.to_le_bytes());
        self.pop_stack(2);
    }

    /// Emits BUILTIN with a function ID.
    /// All builtins pop `arg_count` values and push one result.
    /// The arg count is looked up from `opcode::builtin::arg_count()`.
    pub fn emit_builtin(&mut self, func_id: u16) {
        self.bytecode.push(opcode::BUILTIN);
        self.bytecode.extend_from_slice(&func_id.to_le_bytes());
        // Net effect: pop arg_count, push 1 = pop (arg_count - 1)
        let arg_count = opcode::builtin::arg_count(func_id);
        if arg_count > 1 {
            self.pop_stack(arg_count - 1);
        }
    }

    /// Creates a new unbound label for use as a jump target.
    pub fn create_label(&mut self) -> Label {
        let index = self.labels.len();
        self.labels.push(None);
        Label(index)
    }

    /// Binds a label to the current bytecode position.
    pub fn bind_label(&mut self, label: Label) {
        self.labels[label.0] = Some(self.bytecode.len());
    }

    /// Emits JMP with a placeholder offset targeting the given label.
    pub fn emit_jmp(&mut self, label: Label) {
        self.bytecode.push(opcode::JMP);
        let patch_offset = self.bytecode.len();
        self.bytecode.extend_from_slice(&0i16.to_le_bytes());
        self.patches.push(PendingPatch {
            patch_offset,
            label,
        });
    }

    /// Emits JMP_IF_NOT with a placeholder offset targeting the given label.
    /// Pops the condition value from the stack.
    pub fn emit_jmp_if_not(&mut self, label: Label) {
        self.bytecode.push(opcode::JMP_IF_NOT);
        let patch_offset = self.bytecode.len();
        self.bytecode.extend_from_slice(&0i16.to_le_bytes());
        self.patches.push(PendingPatch {
            patch_offset,
            label,
        });
        self.pop_stack(1);
    }

    /// Emits STR_INIT with data_offset and max_length operands.
    /// Initializes a STRING variable's header in the data region.
    pub fn emit_str_init(&mut self, data_offset: u32, max_length: u16) {
        self.bytecode.push(opcode::STR_INIT);
        self.bytecode.extend_from_slice(&data_offset.to_le_bytes());
        self.bytecode.extend_from_slice(&max_length.to_le_bytes());
        // No stack effect.
    }

    /// Emits LOAD_CONST_STR with a constant pool index.
    /// Loads a string literal from the constant pool into a temp buffer and pushes buf_idx.
    pub fn emit_load_const_str(&mut self, pool_index: u16) {
        self.bytecode.push(opcode::LOAD_CONST_STR);
        self.bytecode.extend_from_slice(&pool_index.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits STR_STORE_VAR with a data_offset operand.
    /// Pops buf_idx from the stack and copies the temp buffer contents to the data region.
    pub fn emit_str_store_var(&mut self, data_offset: u32) {
        self.bytecode.push(opcode::STR_STORE_VAR);
        self.bytecode.extend_from_slice(&data_offset.to_le_bytes());
        self.pop_stack(1);
    }

    /// Emits STR_LOAD_VAR with a data_offset operand.
    /// Copies a string from the data region into a temp buffer and pushes buf_idx.
    #[allow(dead_code)]
    pub fn emit_str_load_var(&mut self, data_offset: u32) {
        self.bytecode.push(opcode::STR_LOAD_VAR);
        self.bytecode.extend_from_slice(&data_offset.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits LEN_STR with a data_offset operand.
    /// Reads the current length of a STRING variable from the data region
    /// and pushes the result as an i32.
    pub fn emit_len_str(&mut self, data_offset: u32) {
        self.bytecode.push(opcode::LEN_STR);
        self.bytecode.extend_from_slice(&data_offset.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits FIND_STR with two data_offset operands.
    /// Finds the first occurrence of IN2 within IN1 and pushes the
    /// 1-based position as an i32 (0 if not found).
    pub fn emit_find_str(&mut self, in1_data_offset: u32, in2_data_offset: u32) {
        self.bytecode.push(opcode::FIND_STR);
        self.bytecode
            .extend_from_slice(&in1_data_offset.to_le_bytes());
        self.bytecode
            .extend_from_slice(&in2_data_offset.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits REPLACE_STR with two data_offset operands.
    /// Pops P (i32) then L (i32) from stack, replaces L characters at
    /// position P in IN1 with IN2, and pushes the result buf_idx.
    pub fn emit_replace_str(&mut self, in1_data_offset: u32, in2_data_offset: u32) {
        self.bytecode.push(opcode::REPLACE_STR);
        self.bytecode
            .extend_from_slice(&in1_data_offset.to_le_bytes());
        self.bytecode
            .extend_from_slice(&in2_data_offset.to_le_bytes());
        // Net effect: pop 2 (L, P), push 1 (buf_idx) = pop 1
        self.pop_stack(1);
    }

    /// Emits INSERT_STR with two data_offset operands.
    /// Pops P (i32) from stack, inserts IN2 into IN1 after position P,
    /// and pushes the result buf_idx.
    pub fn emit_insert_str(&mut self, in1_data_offset: u32, in2_data_offset: u32) {
        self.bytecode.push(opcode::INSERT_STR);
        self.bytecode
            .extend_from_slice(&in1_data_offset.to_le_bytes());
        self.bytecode
            .extend_from_slice(&in2_data_offset.to_le_bytes());
        // Net effect: pop 1 (P), push 1 (buf_idx) = 0
    }

    /// Emits DELETE_STR with a data_offset operand.
    /// Pops P (i32) then L (i32) from stack, deletes L characters from
    /// IN1 starting at position P, and pushes the result buf_idx.
    pub fn emit_delete_str(&mut self, in1_data_offset: u32) {
        self.bytecode.push(opcode::DELETE_STR);
        self.bytecode
            .extend_from_slice(&in1_data_offset.to_le_bytes());
        // Net effect: pop 2 (L, P), push 1 (buf_idx) = pop 1
        self.pop_stack(1);
    }

    /// Emits LEFT_STR with a data_offset operand.
    /// Pops L (i32) from stack, returns the leftmost L characters of IN,
    /// and pushes the result buf_idx.
    pub fn emit_left_str(&mut self, in_data_offset: u32) {
        self.bytecode.push(opcode::LEFT_STR);
        self.bytecode
            .extend_from_slice(&in_data_offset.to_le_bytes());
        // Net effect: pop 1 (L), push 1 (buf_idx) = 0
    }

    /// Emits RIGHT_STR with a data_offset operand.
    /// Pops L (i32) from stack, returns the rightmost L characters of IN,
    /// and pushes the result buf_idx.
    pub fn emit_right_str(&mut self, in_data_offset: u32) {
        self.bytecode.push(opcode::RIGHT_STR);
        self.bytecode
            .extend_from_slice(&in_data_offset.to_le_bytes());
        // Net effect: pop 1 (L), push 1 (buf_idx) = 0
    }

    /// Emits MID_STR with a data_offset operand.
    /// Pops P (i32) then L (i32) from stack, returns L characters from
    /// IN starting at position P, and pushes the result buf_idx.
    pub fn emit_mid_str(&mut self, in_data_offset: u32) {
        self.bytecode.push(opcode::MID_STR);
        self.bytecode
            .extend_from_slice(&in_data_offset.to_le_bytes());
        // Net effect: pop 2 (L, P), push 1 (buf_idx) = pop 1
        self.pop_stack(1);
    }

    /// Emits CONCAT_STR with two data_offset operands.
    /// Concatenates IN1 and IN2, pushes the result buf_idx.
    pub fn emit_concat_str(&mut self, in1_data_offset: u32, in2_data_offset: u32) {
        self.bytecode.push(opcode::CONCAT_STR);
        self.bytecode
            .extend_from_slice(&in1_data_offset.to_le_bytes());
        self.bytecode
            .extend_from_slice(&in2_data_offset.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits POP (discards top of stack).
    pub fn emit_pop(&mut self) {
        self.bytecode.push(opcode::POP);
        self.pop_stack(1);
    }

    /// Emits FB_LOAD_INSTANCE with a variable index.
    pub fn emit_fb_load_instance(&mut self, var_index: VarIndex) {
        self.bytecode.push(opcode::FB_LOAD_INSTANCE);
        self.bytecode.extend_from_slice(&var_index.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits FB_STORE_PARAM with a field index.
    /// Pops one value (the parameter); fb_ref remains on stack.
    pub fn emit_fb_store_param(&mut self, field: u8) {
        self.bytecode.push(opcode::FB_STORE_PARAM);
        self.bytecode.push(field);
        self.pop_stack(1);
    }

    /// Emits FB_LOAD_PARAM with a field index.
    /// Pushes one value (the output parameter); fb_ref remains on stack.
    pub fn emit_fb_load_param(&mut self, field: u8) {
        self.bytecode.push(opcode::FB_LOAD_PARAM);
        self.bytecode.push(field);
        self.push_stack(1);
    }

    /// Emits FB_CALL with a type_id.
    /// Net stack effect: 0 (fb_ref stays on stack).
    pub fn emit_fb_call(&mut self, type_id: u16) {
        self.bytecode.push(opcode::FB_CALL);
        self.bytecode.extend_from_slice(&type_id.to_le_bytes());
    }

    /// Emits CALL with a function ID and variable offset.
    /// Pops `num_params` arguments and pushes one return value.
    /// `var_offset` is the absolute variable table index where the
    /// function's parameters start.
    pub fn emit_call(
        &mut self,
        function_id: u16,
        num_params: u16,
        var_offset: VarIndex,
        callee_max_stack: u16,
    ) {
        self.bytecode.push(opcode::CALL);
        self.bytecode.extend_from_slice(&function_id.to_le_bytes());
        self.bytecode.extend_from_slice(&var_offset.to_le_bytes());
        if num_params > 0 {
            self.pop_stack(num_params);
        }
        // Account for the callee's stack usage on the shared stack.
        // The callee will use up to callee_max_stack slots on top of the
        // current depth, then leave exactly one return value.
        if callee_max_stack > 0 {
            self.push_stack(callee_max_stack);
            self.pop_stack(callee_max_stack);
        }
        self.push_stack(1);
    }

    /// Emits RET (return with value on stack).
    pub fn emit_ret(&mut self) {
        self.bytecode.push(opcode::RET);
    }

    /// Emits RET_VOID.
    pub fn emit_ret_void(&mut self) {
        self.bytecode.push(opcode::RET_VOID);
    }

    /// Returns the accumulated bytecode, resolving all pending jump patches.
    pub fn bytecode(&mut self) -> &[u8] {
        self.patch_jumps();
        &self.bytecode
    }

    /// Returns the maximum stack depth reached during emission.
    pub fn max_stack_depth(&self) -> u16 {
        self.max_stack_depth
    }

    /// Resolves all pending jump patches by computing relative offsets.
    fn patch_jumps(&mut self) {
        for patch in self.patches.drain(..) {
            let label_pos =
                self.labels[patch.label.0].expect("label must be bound before patching");
            // Offset is relative to the byte after the i16 operand
            let next_pc = patch.patch_offset + 2;
            let offset = (label_pos as isize - next_pc as isize) as i16;
            let bytes = offset.to_le_bytes();
            self.bytecode[patch.patch_offset] = bytes[0];
            self.bytecode[patch.patch_offset + 1] = bytes[1];
        }
    }

    fn push_stack(&mut self, count: u16) {
        self.current_stack_depth += count;
        if self.current_stack_depth > self.max_stack_depth {
            self.max_stack_depth = self.current_stack_depth;
        }
    }

    fn pop_stack(&mut self, count: u16) {
        self.current_stack_depth = self.current_stack_depth.saturating_sub(count);
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_when_load_const_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00]);
    }

    #[test]
    fn emitter_when_load_var_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(1));

        assert_eq!(em.bytecode(), &[0x10, 0x01, 0x00]);
    }

    #[test]
    fn emitter_when_store_var_then_correct_bytecode() {
        let mut em = Emitter::new();
        // Need something on the stack first
        em.emit_load_const_i32(0);
        em.emit_store_var_i32(VarIndex::new(0));

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00]);
    }

    #[test]
    fn emitter_when_add_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_add_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x30]);
    }

    #[test]
    fn emitter_when_sub_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_sub_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x31]);
    }

    #[test]
    fn emitter_when_sub_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x - 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_sub_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_mul_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_mul_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x32]);
    }

    #[test]
    fn emitter_when_mul_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x * 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_mul_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_div_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_div_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x33]);
    }

    #[test]
    fn emitter_when_div_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x / 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_div_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_mod_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_mod_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x34]);
    }

    #[test]
    fn emitter_when_mod_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x MOD 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_mod_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_neg_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0));
        em.emit_neg_i32();

        assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0x35]);
    }

    #[test]
    fn emitter_when_neg_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := -x
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_neg_i32(); // stack: 1 (pop 1, push 1)
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_ret_void_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_ret_void();

        assert_eq!(em.bytecode(), &[0xB5]);
    }

    #[test]
    fn emitter_when_steel_thread_then_tracks_max_stack_depth() {
        let mut em = Emitter::new();
        // x := 10
        em.emit_load_const_i32(0); // stack: 1
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0
                                                 // y := x + 32
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(1); // stack: 2
        em.emit_add_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0
        em.emit_ret_void();

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_large_pool_index_then_little_endian() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(256);

        // 256 in little-endian u16 is [0x00, 0x01]
        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x01]);
    }

    #[test]
    fn emitter_when_builtin_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_builtin(opcode::builtin::EXPT_I32);

        // LOAD_CONST pool:0, LOAD_CONST pool:1, BUILTIN 0x0340
        assert_eq!(
            em.bytecode(),
            &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0xC4, 0x40, 0x03]
        );
    }

    #[test]
    fn emitter_when_builtin_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x ** 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_builtin(opcode::builtin::EXPT_I32); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_builtin_1_arg_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := ABS(x)
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_builtin(opcode::builtin::ABS_I32); // stack: 1 (pop 1, push 1)
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_builtin_3_arg_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := LIMIT(mn, x, mx)
        em.emit_load_const_i32(0); // stack: 1
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 2
        em.emit_load_const_i32(1); // stack: 3
        em.emit_builtin(opcode::builtin::LIMIT_I32); // stack: 1 (pop 3, push 1)
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 3);
    }

    #[test]
    fn emitter_when_eq_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_eq_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x68]);
    }

    #[test]
    fn emitter_when_eq_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x = 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_eq_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_ne_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_ne_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x69]);
    }

    #[test]
    fn emitter_when_ne_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x <> 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_ne_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_lt_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_lt_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x6A]);
    }

    #[test]
    fn emitter_when_lt_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x < 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_lt_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_le_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_le_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x6B]);
    }

    #[test]
    fn emitter_when_le_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x <= 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_le_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_gt_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_gt_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x6C]);
    }

    #[test]
    fn emitter_when_gt_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x > 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_gt_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_ge_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_ge_i32();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x6D]);
    }

    #[test]
    fn emitter_when_ge_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := x >= 5
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_ge_i32(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_bool_and_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_bool_and();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x54]);
    }

    #[test]
    fn emitter_when_bool_and_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_var_i32(VarIndex::new(1)); // stack: 2
        em.emit_bool_and(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(2)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_bool_or_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_bool_or();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x55]);
    }

    #[test]
    fn emitter_when_bool_or_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_var_i32(VarIndex::new(1)); // stack: 2
        em.emit_bool_or(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(2)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_bool_xor_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0);
        em.emit_load_const_i32(1);
        em.emit_bool_xor();

        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x56]);
    }

    #[test]
    fn emitter_when_bool_xor_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_load_var_i32(VarIndex::new(1)); // stack: 2
        em.emit_bool_xor(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(2)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_bool_not_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0));
        em.emit_bool_not();

        assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0x57]);
    }

    #[test]
    fn emitter_when_bool_not_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := NOT x
        em.emit_load_var_i32(VarIndex::new(0)); // stack: 1
        em.emit_bool_not(); // stack: 1 (pop 1, push 1)
        em.emit_store_var_i32(VarIndex::new(1)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_load_true_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_true();

        assert_eq!(em.bytecode(), &[0x07]);
    }

    #[test]
    fn emitter_when_load_true_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_true(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_load_false_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_false();

        assert_eq!(em.bytecode(), &[0x08]);
    }

    #[test]
    fn emitter_when_load_false_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_false(); // stack: 1
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_jmp_then_correct_bytecode() {
        let mut em = Emitter::new();
        let label = em.create_label();
        em.emit_jmp(label);
        em.bind_label(label);

        // JMP with offset 0 (target is immediately after the instruction)
        assert_eq!(em.bytecode(), &[0xB0, 0x00, 0x00]);
    }

    #[test]
    fn emitter_when_jmp_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // stack: 1
        let label = em.create_label();
        em.emit_jmp(label);
        em.bind_label(label);
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0

        // JMP does not change stack depth
        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_jmp_if_not_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // push condition
        let label = em.create_label();
        em.emit_jmp_if_not(label);
        em.bind_label(label);

        // LOAD_CONST_I32 pool:0, JMP_IF_NOT offset:0
        assert_eq!(em.bytecode(), &[0x01, 0x00, 0x00, 0xB2, 0x00, 0x00]);
    }

    #[test]
    fn emitter_when_jmp_if_not_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // stack: 1
        let label = em.create_label();
        em.emit_jmp_if_not(label); // stack: 0 (pops condition)
        em.bind_label(label);

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_call_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // arg 1
        em.emit_load_const_i32(1); // arg 2
        em.emit_call(2, 2, VarIndex::new(5), 0); // CALL function 2, 2 params, var_offset=5

        // LOAD_CONST pool:0, LOAD_CONST pool:1, CALL func:2 var_offset:5
        assert_eq!(
            em.bytecode(),
            &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0xB3, 0x02, 0x00, 0x05, 0x00]
        );
    }

    #[test]
    fn emitter_when_call_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // stack: 1
        em.emit_load_const_i32(1); // stack: 2
        em.emit_call(2, 2, VarIndex::new(5), 0); // pop 2 args, push 1 result = stack: 1
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_ret_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(VarIndex::new(0));
        em.emit_ret();

        assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0xB4]);
    }

    #[test]
    fn emitter_when_forward_jump_then_patches_correctly() {
        let mut em = Emitter::new();
        let label = em.create_label();
        em.emit_jmp(label);
        // Emit 4 bytes of filler (a LOAD_CONST_I32)
        em.emit_load_const_i32(0);
        em.bind_label(label);

        // JMP offset should be 3 (skip over the LOAD_CONST_I32 which is 3 bytes)
        assert_eq!(em.bytecode(), &[0xB0, 0x03, 0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn emitter_when_load_array_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // push flat index
        em.emit_load_array(VarIndex::new(3), 7); // var_index=3, desc_index=7

        // LOAD_CONST pool:0, LOAD_ARRAY var:3 desc:7
        assert_eq!(
            em.bytecode(),
            &[0x01, 0x00, 0x00, 0x24, 0x03, 0x00, 0x07, 0x00]
        );
    }

    #[test]
    fn emitter_when_load_array_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // stack: 1 (flat index)
        em.emit_load_array(VarIndex::new(3), 7); // stack: 1 (pop index, push value)
        em.emit_store_var_i32(VarIndex::new(0)); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }

    #[test]
    fn emitter_when_store_array_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // push value
        em.emit_load_const_i32(1); // push flat index
        em.emit_store_array(VarIndex::new(5), 2); // var_index=5, desc_index=2

        // LOAD_CONST pool:0, LOAD_CONST pool:1, STORE_ARRAY var:5 desc:2
        assert_eq!(
            em.bytecode(),
            &[0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x25, 0x05, 0x00, 0x02, 0x00]
        );
    }

    #[test]
    fn emitter_when_store_array_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        em.emit_load_const_i32(0); // stack: 1 (value)
        em.emit_load_const_i32(1); // stack: 2 (flat index)
        em.emit_store_array(VarIndex::new(5), 2); // stack: 0 (pops 2)

        assert_eq!(em.max_stack_depth(), 2);
    }
}
