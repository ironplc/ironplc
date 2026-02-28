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
}
