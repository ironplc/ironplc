//! Tests for the peephole optimizer pipeline.
//!
//! These exercise the observable behaviour of [`optimize`] — the bytes it
//! produces and the jump offsets it rewrites — rather than any individual
//! pass, so a change to how the passes are arranged cannot quietly change
//! what the optimizer does.

use ironplc_container::opcode;

use super::{optimize, remap_line_map};
use crate::compile::PoolConstant;
use crate::emit::EmittedLineMapEntry;

fn line_entry(bytecode_offset: u16, source_line: u16) -> EmittedLineMapEntry {
    EmittedLineMapEntry {
        bytecode_offset,
        file_id: ironplc_container::SourceFileId::new(0),
        source_line: ironplc_container::SourceLine::new(source_line),
        source_column: ironplc_container::SourceColumn::new(1),
    }
}

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

/// Builds a `CMP_BR_I32`: opcode + cmp_op + var_idx + const_idx + offset.
fn cmp_br_i32(var_idx: u16, const_idx: u16, offset: i16) -> Vec<u8> {
    let mut v = vec![opcode::CMP_BR_I32, opcode::cmp_op::LE_S];
    v.extend_from_slice(&var_idx.to_le_bytes());
    v.extend_from_slice(&const_idx.to_le_bytes());
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
    v.push(1); // char_width (narrow)
    v
}

#[test]
fn optimize_when_empty_bytecode_then_returns_empty() {
    let (result, _) = optimize(&[], &mut vec![]);
    assert!(result.is_empty());
}

#[test]
fn optimize_when_no_patterns_then_bytecode_unchanged() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.extend_from_slice(&load_const_i32(1));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(10), PoolConstant::I32(20)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, bytecode);
}

// --- Pattern 1: LOAD_VAR + STORE_VAR same var ---

#[test]
fn optimize_when_load_store_same_var_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_store_same_var_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i64(3));
    bytecode.extend_from_slice(&store_var_i64(3));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_store_different_var_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(6));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_store_different_type_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i64(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

// --- Pattern 2: LOAD_CONST(0) + ADD/SUB ---

#[test]
fn optimize_when_load_const_zero_add_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_sub_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::SUB_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_add_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i64(0));
    bytecode.push(opcode::ADD_I64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I64(0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_add_f32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f32(0));
    bytecode.push(opcode::ADD_F32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F32(0.0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_add_f64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f64(0));
    bytecode.push(opcode::ADD_F64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F64(0.0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_nonzero_add_i32_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(42)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, bytecode);
}

// --- Pattern 3: LOAD_CONST(1) + MUL/DIV ---

#[test]
fn optimize_when_load_const_one_mul_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::MUL_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(1)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_div_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::DIV_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(1)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i64(0));
    bytecode.push(opcode::MUL_I64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I64(1)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_f32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f32(0));
    bytecode.push(opcode::MUL_F32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F32(1.0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_f64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f64(0));
    bytecode.push(opcode::MUL_F64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F64(1.0)];
    let (result, _) = optimize(&bytecode, &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_nonone_mul_i32_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::MUL_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(5)];
    let (result, _) = optimize(&bytecode, &mut constants);
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

    let (result, _) = optimize(&bytecode, &mut vec![]);
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

    let (result, _) = optimize(&bytecode, &mut vec![]);

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
fn optimize_when_cmp_br_over_removed_instructions_then_adjusts_offset() {
    // A CMP_BR branching forward over a removable pair. Before this was
    // handled, the offset was left stale and the branch landed inside an
    // instruction, decoding as garbage at run time.
    //
    // Layout:
    //   [0]  CMP_BR_I32 +7    -> targets offset 15 (RET_VOID)
    //   [8]  LOAD_VAR_I32 5   ]
    //   [11] STORE_VAR_I32 5  ]-- removable pair
    //   [14] LOAD_TRUE
    //   [15] RET_VOID         <- branch target
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&cmp_br_i32(1, 2, 7));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);

    // After removing 6 bytes, the branch must still land on RET_VOID:
    //   [0] CMP_BR_I32 +1
    //   [8] LOAD_TRUE
    //   [9] RET_VOID
    let mut expected = Vec::new();
    expected.extend_from_slice(&cmp_br_i32(1, 2, 1));
    expected.push(opcode::LOAD_TRUE);
    expected.push(opcode::RET_VOID);

    assert_eq!(result, expected);
}

#[test]
fn optimize_when_cmp_br_targets_removable_pair_then_pair_is_kept() {
    // The branch target must be protected from removal, otherwise the
    // branch would land on whatever instruction followed the pair.
    //
    // Layout:
    //   [0]  LOAD_TRUE
    //   [1]  CMP_BR_I32 +0    -> targets offset 9 (the LOAD_VAR_I32)
    //   [9]  LOAD_VAR_I32 5   ]
    //   [12] STORE_VAR_I32 5  ]-- removable, but [9] is a branch target
    //   [15] RET_VOID
    let mut bytecode = Vec::new();
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.extend_from_slice(&cmp_br_i32(1, 2, 0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);

    assert_eq!(result, bytecode, "branch target must not be removed");
}

#[test]
fn optimize_when_cmp_br_branches_backward_then_adjusts_offset() {
    // The loop shape: a removable pair inside the body, with the branch
    // jumping backwards over it.
    //
    // Layout:
    //   [0]  LOAD_TRUE        <- branch target
    //   [1]  LOAD_VAR_I32 5   ]
    //   [4]  STORE_VAR_I32 5  ]-- removable pair
    //   [7]  CMP_BR_I32 -15   -> targets offset 0
    //   [15] RET_VOID
    let mut bytecode = Vec::new();
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.extend_from_slice(&cmp_br_i32(1, 2, -15));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);

    //   [0] LOAD_TRUE
    //   [1] CMP_BR_I32 -9
    //   [9] RET_VOID
    let mut expected = Vec::new();
    expected.push(opcode::LOAD_TRUE);
    expected.extend_from_slice(&cmp_br_i32(1, 2, -9));
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

    let mut constants = vec![PoolConstant::I32(0)];
    let (result, _) = optimize(&bytecode, &mut constants);
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

    let (result, _) = optimize(&bytecode, &mut vec![]);
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

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_str_init_before_jump_then_no_panic() {
    // STR_INIT uses u32 + u16 + u8 operands (8 bytes total). A wrong
    // instruction size would desynchronize the decoder.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&str_init(100, 80));
    bytecode.extend_from_slice(&jmp(1));
    bytecode.push(opcode::POP);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_const_out_of_bounds_add_i32_then_keeps_instructions() {
    // Drives is_zero_constant's `_ => false` default arm: the pool index
    // points past the end of the empty constant pool.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(5));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_const_out_of_bounds_mul_i32_then_keeps_instructions() {
    // Drives is_one_constant's `_ => false` default arm.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(5));
    bytecode.push(opcode::MUL_I32);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(&bytecode, &mut vec![]);
    assert_eq!(result, bytecode);
}

// --- Offset-map composition across passes ---
//
// Each pass produces its own old->new map and the driver folds them
// together. These pin the fold, which no single-pass test can reach:
// the shared bytecode below has one pair removed by pass_self_assign
// and a second pair removed by pass_arith_identity.

/// Layout:
///   [0]  JMP +11          -> targets offset 14 (RET_VOID)
///   [3]  LOAD_VAR_I32 5   ]-- removed by pass_self_assign
///   [6]  STORE_VAR_I32 5  ]
///   [9]  LOAD_CONST_I32 0 ]-- removed by pass_arith_identity
///   [12] ADD_I32          ]
///   [13] LOAD_TRUE
///   [14] RET_VOID         <- jump target
fn two_pass_bytecode() -> Vec<u8> {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&jmp(11));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.push(opcode::RET_VOID);
    bytecode
}

#[test]
fn optimize_when_jump_spans_removals_from_two_passes_then_adjusts_offset() {
    let (result, _) = optimize(&two_pass_bytecode(), &mut vec![PoolConstant::I32(0)]);

    // Ten bytes removed across the two passes, so the target moves from
    // 14 to 4:
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
fn remap_line_map_when_entry_removed_by_first_pass_then_snaps_past_second_pass() {
    let bytecode = two_pass_bytecode();
    let (result, offset_map) = optimize(&bytecode, &mut vec![PoolConstant::I32(0)]);

    // Offset 3 is the LOAD_VAR the first pass removes; offset 14 is the
    // RET_VOID that survives both passes.
    let raw = vec![line_entry(3, 10), line_entry(14, 20)];
    let remapped = remap_line_map(raw, &offset_map, result.len() as u16)
        .expect("every entry sits on an instruction boundary");

    // The entry on the removed instruction snaps forward past the second
    // pass's removal onto LOAD_TRUE at offset 3, and the surviving
    // instruction's entry follows it at offset 4.
    assert_eq!(
        remapped,
        vec![line_entry(3, 10), line_entry(4, 20)],
        "composed map must snap forward through both passes"
    );
}

#[test]
fn remap_line_map_when_entry_is_not_an_instruction_boundary_then_internal_error() {
    // The offset map covers every instruction boundary, so an entry that
    // misses it means the emitter recorded a position mid-instruction. That
    // is a compiler defect rather than anything the program being compiled
    // can cause, so it is reported instead of dropped.
    let bytecode = two_pass_bytecode();
    let (result, offset_map) = optimize(&bytecode, &mut vec![PoolConstant::I32(0)]);

    let raw = vec![line_entry(4, 10)];
    let diagnostic = remap_line_map(raw, &offset_map, result.len() as u16).unwrap_err();

    assert_eq!(diagnostic.code, "P9998");
    assert!(diagnostic
        .primary
        .message
        .contains("not an instruction boundary"));
}
