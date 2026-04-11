//! Post-emission peephole optimizer for bytecode.
//!
//! Runs a single pass over the raw bytecode buffer produced by the emitter,
//! matching adjacent instruction pairs against identity/no-op patterns and
//! removing them. Jump offsets are adjusted to account for removed bytes.
//!
//! This pass runs after the emitter's own in-line peephole optimizations
//! (consecutive load -> DUP, store-load -> DUP+STORE) and complements
//! them by handling arithmetic identities that are only visible once the
//! full instruction stream exists.
//!
//! Patterns recognized:
//!
//! 1. `LOAD_VAR x; STORE_VAR x` (same var, same type) — redundant
//!    self-assignment, both removed.
//! 2. `LOAD_CONST 0; ADD|SUB` (matching width) — additive identity,
//!    both removed.
//! 3. `LOAD_CONST 1; MUL|DIV` (matching width) — multiplicative identity,
//!    both removed.
//!
//! Instructions that are the target of a jump are never removed; this
//! preserves basic-block boundaries and guarantees jump targets always
//! map to a valid new offset.

use std::collections::{HashMap, HashSet};

use ironplc_container::opcode;

use crate::compile::PoolConstant;

/// A decoded instruction: its original byte offset and raw bytes.
struct Instruction {
    offset: usize,
    bytes: Vec<u8>,
}

impl Instruction {
    fn opcode(&self) -> u8 {
        self.bytes[0]
    }

    fn u16_operand(&self) -> u16 {
        u16::from_le_bytes([self.bytes[1], self.bytes[2]])
    }
}

/// Decode raw bytecode into a list of instructions and the set of jump
/// target offsets (relative to the original bytecode).
fn decode(bytecode: &[u8]) -> (Vec<Instruction>, HashSet<usize>) {
    let mut instructions = Vec::new();
    let mut jump_targets = HashSet::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        let size = opcode::instruction_size(op);
        let end = (pc + size).min(bytecode.len());
        instructions.push(Instruction {
            offset: pc,
            bytes: bytecode[pc..end].to_vec(),
        });

        if (op == opcode::JMP || op == opcode::JMP_IF_NOT) && end - pc >= 3 {
            let rel = i16::from_le_bytes([bytecode[pc + 1], bytecode[pc + 2]]);
            let target = (pc as isize + 3 + rel as isize) as usize;
            jump_targets.insert(target);
        }

        pc = end;
    }

    (instructions, jump_targets)
}

/// Returns true if the numeric constant at `pool_index` is zero.
fn is_zero_constant(constants: &[PoolConstant], pool_index: u16) -> bool {
    match constants.get(pool_index as usize) {
        Some(PoolConstant::I32(v)) => *v == 0,
        Some(PoolConstant::I64(v)) => *v == 0,
        Some(PoolConstant::F32(v)) => *v == 0.0,
        Some(PoolConstant::F64(v)) => *v == 0.0,
        _ => false,
    }
}

/// Returns true if the numeric constant at `pool_index` is one.
fn is_one_constant(constants: &[PoolConstant], pool_index: u16) -> bool {
    match constants.get(pool_index as usize) {
        Some(PoolConstant::I32(v)) => *v == 1,
        Some(PoolConstant::I64(v)) => *v == 1,
        Some(PoolConstant::F32(v)) => *v == 1.0,
        Some(PoolConstant::F64(v)) => *v == 1.0,
        _ => false,
    }
}

/// Returns the matching STORE opcode for a given LOAD_VAR opcode, or None.
fn matching_store_for_load(load_op: u8) -> Option<u8> {
    match load_op {
        opcode::LOAD_VAR_I32 => Some(opcode::STORE_VAR_I32),
        opcode::LOAD_VAR_I64 => Some(opcode::STORE_VAR_I64),
        opcode::LOAD_VAR_F32 => Some(opcode::STORE_VAR_F32),
        opcode::LOAD_VAR_F64 => Some(opcode::STORE_VAR_F64),
        _ => None,
    }
}

/// Returns the (ADD, SUB) opcodes for a given LOAD_CONST opcode width, or None.
fn additive_ops_for_const(const_op: u8) -> Option<(u8, u8)> {
    match const_op {
        opcode::LOAD_CONST_I32 => Some((opcode::ADD_I32, opcode::SUB_I32)),
        opcode::LOAD_CONST_I64 => Some((opcode::ADD_I64, opcode::SUB_I64)),
        opcode::LOAD_CONST_F32 => Some((opcode::ADD_F32, opcode::SUB_F32)),
        opcode::LOAD_CONST_F64 => Some((opcode::ADD_F64, opcode::SUB_F64)),
        _ => None,
    }
}

/// Returns the (MUL, DIV) opcodes for a given LOAD_CONST opcode width, or None.
fn multiplicative_ops_for_const(const_op: u8) -> Option<(u8, u8)> {
    match const_op {
        opcode::LOAD_CONST_I32 => Some((opcode::MUL_I32, opcode::DIV_I32)),
        opcode::LOAD_CONST_I64 => Some((opcode::MUL_I64, opcode::DIV_I64)),
        opcode::LOAD_CONST_F32 => Some((opcode::MUL_F32, opcode::DIV_F32)),
        opcode::LOAD_CONST_F64 => Some((opcode::MUL_F64, opcode::DIV_F64)),
        _ => None,
    }
}

/// Returns true if the pair `(a, b)` matches a removable identity pattern.
fn is_removable_pair(a: &Instruction, b: &Instruction, constants: &[PoolConstant]) -> bool {
    let a_op = a.opcode();
    let b_op = b.opcode();

    // Pattern 1: LOAD_VAR + STORE_VAR same var, same type.
    if let Some(expected_store) = matching_store_for_load(a_op) {
        if b_op == expected_store && a.bytes.len() == 3 && b.bytes.len() == 3 {
            let a_var = a.u16_operand();
            let b_var = b.u16_operand();
            if a_var == b_var {
                return true;
            }
        }
    }

    // Pattern 2: LOAD_CONST(0) + ADD/SUB of matching width.
    if let Some((add_op, sub_op)) = additive_ops_for_const(a_op) {
        if (b_op == add_op || b_op == sub_op) && a.bytes.len() == 3 {
            let pool_idx = a.u16_operand();
            if is_zero_constant(constants, pool_idx) {
                return true;
            }
        }
    }

    // Pattern 3: LOAD_CONST(1) + MUL/DIV of matching width.
    if let Some((mul_op, div_op)) = multiplicative_ops_for_const(a_op) {
        if (b_op == mul_op || b_op == div_op) && a.bytes.len() == 3 {
            let pool_idx = a.u16_operand();
            if is_one_constant(constants, pool_idx) {
                return true;
            }
        }
    }

    false
}

/// Runs the peephole optimizer on `bytecode`.
///
/// Returns a new byte vector with removable identity patterns stripped and
/// jump offsets adjusted. If no patterns are found, the output is equal to
/// the input.
pub(crate) fn optimize(bytecode: &[u8], constants: &[PoolConstant]) -> Vec<u8> {
    if bytecode.is_empty() {
        return Vec::new();
    }

    let (instructions, jump_targets) = decode(bytecode);

    // First pass: mark instructions that are part of a removable pair.
    let mut removed = vec![false; instructions.len()];
    let mut i = 0;
    while i + 1 < instructions.len() {
        let a = &instructions[i];
        let b = &instructions[i + 1];

        // Never touch instructions that are the target of a jump.
        if jump_targets.contains(&a.offset) || jump_targets.contains(&b.offset) {
            i += 1;
            continue;
        }

        if is_removable_pair(a, b, constants) {
            removed[i] = true;
            removed[i + 1] = true;
            i += 2;
        } else {
            i += 1;
        }
    }

    // Build an old-offset -> new-offset map covering every instruction and
    // the one-past-the-end position (used when a jump's target equals the
    // end of the function).
    let mut offset_map: HashMap<usize, usize> = HashMap::new();
    let mut new_offset = 0usize;
    for (idx, instr) in instructions.iter().enumerate() {
        offset_map.insert(instr.offset, new_offset);
        if !removed[idx] {
            new_offset += instr.bytes.len();
        }
    }
    offset_map.insert(bytecode.len(), new_offset);

    // Second pass: rebuild bytecode, rewriting jump offsets.
    let mut output = Vec::with_capacity(bytecode.len());
    for (idx, instr) in instructions.iter().enumerate() {
        if removed[idx] {
            continue;
        }
        let op = instr.opcode();
        if (op == opcode::JMP || op == opcode::JMP_IF_NOT) && instr.bytes.len() == 3 {
            let old_rel = i16::from_le_bytes([instr.bytes[1], instr.bytes[2]]);
            let old_target = (instr.offset as isize + 3 + old_rel as isize) as usize;
            let new_pos = output.len();
            let new_target = offset_map[&old_target];
            let new_rel = (new_target as isize - (new_pos as isize + 3)) as i16;
            output.push(op);
            output.extend_from_slice(&new_rel.to_le_bytes());
        } else {
            output.extend_from_slice(&instr.bytes);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_const_i32(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_CONST_I32];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn load_const_i64(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_CONST_I64];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn load_const_f32(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_CONST_F32];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn load_const_f64(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_CONST_F64];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn load_var_i32(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_VAR_I32];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn store_var_i32(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::STORE_VAR_I32];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn load_var_i64(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_VAR_I64];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn store_var_i64(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::STORE_VAR_I64];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    fn jmp(offset: i16) -> Vec<u8> {
        let mut v = vec![opcode::JMP];
        v.extend_from_slice(&offset.to_le_bytes());
        v
    }

    fn str_load_var(data_offset: u32) -> Vec<u8> {
        let mut v = vec![opcode::STR_LOAD_VAR];
        v.extend_from_slice(&data_offset.to_le_bytes());
        v
    }

    fn find_str(in1: u32, in2: u32) -> Vec<u8> {
        let mut v = vec![opcode::FIND_STR];
        v.extend_from_slice(&in1.to_le_bytes());
        v.extend_from_slice(&in2.to_le_bytes());
        v
    }

    fn str_init(data_offset: u32, max_length: u16) -> Vec<u8> {
        let mut v = vec![opcode::STR_INIT];
        v.extend_from_slice(&data_offset.to_le_bytes());
        v.extend_from_slice(&max_length.to_le_bytes());
        v
    }

    #[test]
    fn optimize_when_empty_bytecode_then_returns_empty() {
        let result = optimize(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn optimize_when_no_patterns_then_bytecode_unchanged() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.extend_from_slice(&load_const_i32(1));
        bytecode.push(opcode::ADD_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(10), PoolConstant::I32(20)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, bytecode);
    }

    // --- Pattern 1: LOAD_VAR + STORE_VAR same var ---

    #[test]
    fn optimize_when_load_store_same_var_i32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_var_i32(5));
        bytecode.extend_from_slice(&store_var_i32(5));
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_store_same_var_i64_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_var_i64(3));
        bytecode.extend_from_slice(&store_var_i64(3));
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_store_different_var_then_no_change() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_var_i32(5));
        bytecode.extend_from_slice(&store_var_i32(6));
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }

    #[test]
    fn optimize_when_load_store_different_type_then_no_change() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_var_i32(5));
        bytecode.extend_from_slice(&store_var_i64(5));
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }

    // --- Pattern 2: LOAD_CONST(0) + ADD/SUB ---

    #[test]
    fn optimize_when_load_const_zero_add_i32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::ADD_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_zero_sub_i32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::SUB_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_zero_add_i64_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i64(0));
        bytecode.push(opcode::ADD_I64);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I64(0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_zero_add_f32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_f32(0));
        bytecode.push(opcode::ADD_F32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::F32(0.0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_zero_add_f64_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_f64(0));
        bytecode.push(opcode::ADD_F64);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::F64(0.0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_nonzero_add_i32_then_no_change() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::ADD_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(42)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, bytecode);
    }

    // --- Pattern 3: LOAD_CONST(1) + MUL/DIV ---

    #[test]
    fn optimize_when_load_const_one_mul_i32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::MUL_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(1)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_one_div_i32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::DIV_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(1)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_one_mul_i64_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i64(0));
        bytecode.push(opcode::MUL_I64);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I64(1)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_one_mul_f32_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_f32(0));
        bytecode.push(opcode::MUL_F32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::F32(1.0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_one_mul_f64_then_removes_both() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_f64(0));
        bytecode.push(opcode::MUL_F64);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::F64(1.0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    #[test]
    fn optimize_when_load_const_nonone_mul_i32_then_no_change() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::MUL_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(5)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, bytecode);
    }

    // --- Jump safety ---

    #[test]
    fn optimize_when_jump_target_then_skips_optimization() {
        // JMP forward past a LOAD_VAR, where the STORE_VAR is the jump target.
        // The pair must NOT be optimized because STORE_VAR is targeted.
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&jmp(3));
        bytecode.extend_from_slice(&load_var_i32(5));
        bytecode.extend_from_slice(&store_var_i32(5));
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }

    #[test]
    fn optimize_when_jump_over_removed_instructions_then_adjusts_offset() {
        // Layout:
        //   [0] JMP +7           -> targets offset 10 (RET_VOID)
        //   [3] LOAD_VAR_I32 5   ]
        //   [6] STORE_VAR_I32 5  ]-- removable pair
        //   [9] LOAD_TRUE
        //   [10] RET_VOID        <- jump target
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&jmp(7));
        bytecode.extend_from_slice(&load_var_i32(5));
        bytecode.extend_from_slice(&store_var_i32(5));
        bytecode.push(opcode::LOAD_TRUE);
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);

        // After removing 6 bytes, new layout:
        //   [0] JMP +1
        //   [3] LOAD_TRUE
        //   [4] RET_VOID
        let mut expected = Vec::new();
        expected.extend_from_slice(&jmp(1));
        expected.push(opcode::LOAD_TRUE);
        expected.push(opcode::RET_VOID);

        assert_eq!(result, expected);
    }

    #[test]
    fn optimize_when_multiple_patterns_then_removes_all() {
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&load_var_i32(1));
        bytecode.extend_from_slice(&store_var_i32(1));
        bytecode.extend_from_slice(&load_const_i32(0));
        bytecode.push(opcode::ADD_I32);
        bytecode.push(opcode::RET_VOID);

        let constants = vec![PoolConstant::I32(0)];
        let result = optimize(&bytecode, &constants);
        assert_eq!(result, vec![opcode::RET_VOID]);
    }

    // --- String opcode regression tests (instruction size correctness) ---

    #[test]
    fn optimize_when_str_load_var_before_jump_then_no_panic() {
        // STR_LOAD_VAR uses a u32 operand (5 bytes total). A wrong
        // instruction size would desynchronize the decoder and cause
        // a panic when resolving the jump target.
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&str_load_var(100));
        bytecode.push(opcode::POP);
        bytecode.extend_from_slice(&jmp(1));
        bytecode.push(opcode::POP);
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }

    #[test]
    fn optimize_when_find_str_before_jump_then_no_panic() {
        // FIND_STR uses two u32 operands (9 bytes total). A wrong
        // instruction size would desynchronize the decoder.
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&find_str(100, 200));
        bytecode.push(opcode::POP);
        bytecode.extend_from_slice(&jmp(1));
        bytecode.push(opcode::POP);
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }

    #[test]
    fn optimize_when_str_init_before_jump_then_no_panic() {
        // STR_INIT uses u32 + u16 operands (7 bytes total). A wrong
        // instruction size would desynchronize the decoder.
        let mut bytecode = Vec::new();
        bytecode.extend_from_slice(&str_init(100, 80));
        bytecode.extend_from_slice(&jmp(1));
        bytecode.push(opcode::POP);
        bytecode.push(opcode::RET_VOID);

        let result = optimize(&bytecode, &[]);
        assert_eq!(result, bytecode);
    }
}
