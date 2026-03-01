//! Low-level bytecode emitter.
//!
//! Provides a builder that appends opcodes and operands to a byte buffer.

use ironplc_container::opcode;

/// Accumulates bytecode instructions.
pub struct Emitter {
    bytecode: Vec<u8>,
    max_stack_depth: u16,
    current_stack_depth: u16,
}

impl Emitter {
    pub fn new() -> Self {
        Emitter {
            bytecode: Vec::new(),
            max_stack_depth: 0,
            current_stack_depth: 0,
        }
    }

    /// Emits LOAD_CONST_I32 with a constant pool index.
    pub fn emit_load_const_i32(&mut self, pool_index: u16) {
        self.bytecode.push(opcode::LOAD_CONST_I32);
        self.bytecode.extend_from_slice(&pool_index.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits LOAD_TRUE (pushes I32 value 1).
    pub fn emit_load_true(&mut self) {
        self.bytecode.push(opcode::LOAD_TRUE);
        self.push_stack(1);
    }

    /// Emits LOAD_FALSE (pushes I32 value 0).
    pub fn emit_load_false(&mut self) {
        self.bytecode.push(opcode::LOAD_FALSE);
        self.push_stack(1);
    }

    /// Emits LOAD_VAR_I32 with a variable table index.
    pub fn emit_load_var_i32(&mut self, var_index: u16) {
        self.bytecode.push(opcode::LOAD_VAR_I32);
        self.bytecode.extend_from_slice(&var_index.to_le_bytes());
        self.push_stack(1);
    }

    /// Emits STORE_VAR_I32 with a variable table index.
    pub fn emit_store_var_i32(&mut self, var_index: u16) {
        self.bytecode.push(opcode::STORE_VAR_I32);
        self.bytecode.extend_from_slice(&var_index.to_le_bytes());
        self.pop_stack(1);
    }

    /// Emits ADD_I32 (pops two, pushes one).
    pub fn emit_add_i32(&mut self) {
        self.bytecode.push(opcode::ADD_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits SUB_I32 (pops two, pushes one).
    pub fn emit_sub_i32(&mut self) {
        self.bytecode.push(opcode::SUB_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits MUL_I32 (pops two, pushes one).
    pub fn emit_mul_i32(&mut self) {
        self.bytecode.push(opcode::MUL_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits DIV_I32 (pops two, pushes one).
    pub fn emit_div_i32(&mut self) {
        self.bytecode.push(opcode::DIV_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits MOD_I32 (pops two, pushes one).
    pub fn emit_mod_i32(&mut self) {
        self.bytecode.push(opcode::MOD_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits NEG_I32 (pops one, pushes one).
    pub fn emit_neg_i32(&mut self) {
        self.bytecode.push(opcode::NEG_I32);
        // Net effect: pop 1, push 1 = no change to stack depth
    }

    /// Emits EQ_I32 (pops two, pushes one).
    pub fn emit_eq_i32(&mut self) {
        self.bytecode.push(opcode::EQ_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits NE_I32 (pops two, pushes one).
    pub fn emit_ne_i32(&mut self) {
        self.bytecode.push(opcode::NE_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits LT_I32 (pops two, pushes one).
    pub fn emit_lt_i32(&mut self) {
        self.bytecode.push(opcode::LT_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits LE_I32 (pops two, pushes one).
    pub fn emit_le_i32(&mut self) {
        self.bytecode.push(opcode::LE_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits GT_I32 (pops two, pushes one).
    pub fn emit_gt_i32(&mut self) {
        self.bytecode.push(opcode::GT_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits GE_I32 (pops two, pushes one).
    pub fn emit_ge_i32(&mut self) {
        self.bytecode.push(opcode::GE_I32);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits BOOL_AND (pops two, pushes one).
    pub fn emit_bool_and(&mut self) {
        self.bytecode.push(opcode::BOOL_AND);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits BOOL_OR (pops two, pushes one).
    pub fn emit_bool_or(&mut self) {
        self.bytecode.push(opcode::BOOL_OR);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits BOOL_XOR (pops two, pushes one).
    pub fn emit_bool_xor(&mut self) {
        self.bytecode.push(opcode::BOOL_XOR);
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits BOOL_NOT (pops one, pushes one).
    pub fn emit_bool_not(&mut self) {
        self.bytecode.push(opcode::BOOL_NOT);
        // Net effect: pop 1, push 1 = no change to stack depth
    }

    /// Emits BUILTIN with a function ID (pops two, pushes one for 2-arg functions).
    pub fn emit_builtin(&mut self, func_id: u16) {
        self.bytecode.push(opcode::BUILTIN);
        self.bytecode.extend_from_slice(&func_id.to_le_bytes());
        // Net effect: pop 2, push 1 = pop 1
        self.pop_stack(1);
    }

    /// Emits RET_VOID.
    pub fn emit_ret_void(&mut self) {
        self.bytecode.push(opcode::RET_VOID);
    }

    /// Returns the accumulated bytecode.
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }

    /// Returns the maximum stack depth reached during emission.
    pub fn max_stack_depth(&self) -> u16 {
        self.max_stack_depth
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
        em.emit_load_var_i32(1);

        assert_eq!(em.bytecode(), &[0x10, 0x01, 0x00]);
    }

    #[test]
    fn emitter_when_store_var_then_correct_bytecode() {
        let mut em = Emitter::new();
        // Need something on the stack first
        em.emit_load_const_i32(0);
        em.emit_store_var_i32(0);

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_sub_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_mul_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_div_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_mod_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_neg_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(0);
        em.emit_neg_i32();

        assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0x35]);
    }

    #[test]
    fn emitter_when_neg_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := -x
        em.emit_load_var_i32(0); // stack: 1
        em.emit_neg_i32(); // stack: 1 (pop 1, push 1)
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_store_var_i32(0); // stack: 0
                                  // y := x + 32
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(1); // stack: 2
        em.emit_add_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0
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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_builtin(opcode::builtin::EXPT_I32); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_eq_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_ne_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_lt_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_le_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_gt_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_const_i32(0); // stack: 2
        em.emit_ge_i32(); // stack: 1
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_var_i32(1); // stack: 2
        em.emit_bool_and(); // stack: 1
        em.emit_store_var_i32(2); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_var_i32(1); // stack: 2
        em.emit_bool_or(); // stack: 1
        em.emit_store_var_i32(2); // stack: 0

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
        em.emit_load_var_i32(0); // stack: 1
        em.emit_load_var_i32(1); // stack: 2
        em.emit_bool_xor(); // stack: 1
        em.emit_store_var_i32(2); // stack: 0

        assert_eq!(em.max_stack_depth(), 2);
    }

    #[test]
    fn emitter_when_bool_not_then_correct_bytecode() {
        let mut em = Emitter::new();
        em.emit_load_var_i32(0);
        em.emit_bool_not();

        assert_eq!(em.bytecode(), &[0x10, 0x00, 0x00, 0x57]);
    }

    #[test]
    fn emitter_when_bool_not_then_tracks_stack_depth() {
        let mut em = Emitter::new();
        // y := NOT x
        em.emit_load_var_i32(0); // stack: 1
        em.emit_bool_not(); // stack: 1 (pop 1, push 1)
        em.emit_store_var_i32(1); // stack: 0

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
        em.emit_store_var_i32(0); // stack: 0

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
        em.emit_store_var_i32(0); // stack: 0

        assert_eq!(em.max_stack_depth(), 1);
    }
}
