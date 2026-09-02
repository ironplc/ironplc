//! Tests for the peephole optimizer pipeline.
//!
//! These exercise the observable behaviour of [`optimize`] — the bytes it
//! produces and where it reports each instruction moved to — rather than any
//! individual pass, so a change to how the passes are arranged cannot quietly
//! change what the optimizer does.
//!
//! The optimizer runs on un-patched bytecode, so every branch operand in the
//! inputs below is the emitter's `0` placeholder and the offsets the branches
//! target are supplied alongside the bytes. The tests that care about the
//! resulting branch offsets run the real emitter through the whole finalize
//! sequence — optimize, then patch — under "Jump patching after optimization".

use std::collections::HashSet;

use ironplc_container::{opcode, VarIndex};

use super::{optimize, remap_line_map, OffsetMap};
use crate::compile::PoolConstant;
use crate::emit::{EmittedLineMapEntry, Emitter, UnpatchedCode};

/// Bytecode with no branches in it.
fn unpatched(bytecode: &[u8]) -> UnpatchedCode<'_> {
    UnpatchedCode {
        bytecode,
        jump_targets: HashSet::new(),
    }
}

/// Bytecode whose branches land on `targets`.
fn unpatched_targeting<'a>(bytecode: &'a [u8], targets: &[usize]) -> UnpatchedCode<'a> {
    UnpatchedCode {
        bytecode,
        jump_targets: targets.iter().copied().collect(),
    }
}

/// Runs what `finalize_function` runs: the optimizer over the emitter's
/// un-patched bytes, then the emitter's own jump patching against the new
/// positions. Returns the bytes that would be stored in the container.
fn optimize_and_patch(emitter: &mut Emitter, constants: &mut Vec<PoolConstant>) -> Vec<u8> {
    let (optimized, offset_map) = optimize(emitter.unpatched_code(), constants);
    emitter.apply_optimized(optimized, &offset_map);
    emitter.bytecode().to_vec()
}

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

/// Builds a `JMP`. Pass `0` for the offset to match what the emitter has
/// written before its patches are resolved.
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
    let (result, offset_map) = optimize(unpatched(&[]), &mut vec![]);
    assert!(result.is_empty());
    assert_eq!(
        offset_map[&0], 0,
        "a label bound past the last instruction must still remap"
    );
}

#[test]
fn optimize_when_no_patterns_then_bytecode_unchanged() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.extend_from_slice(&load_const_i32(1));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(10), PoolConstant::I32(20)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, bytecode);
}

// --- Pattern 1: LOAD_VAR + STORE_VAR same var ---

#[test]
fn optimize_when_load_store_same_var_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_store_same_var_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i64(3));
    bytecode.extend_from_slice(&store_var_i64(3));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_store_different_var_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(6));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_store_different_type_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i64(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
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
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_sub_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::SUB_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_add_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i64(0));
    bytecode.push(opcode::ADD_I64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I64(0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_sub_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i64(0));
    bytecode.push(opcode::SUB_I64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I64(0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

// `x + 0.0` is not an identity on floats: `(-0.0) + 0.0 = +0.0`, so the
// pair must survive. `x - 0.0` is an identity for every `x`, `-0.0` included.

#[test]
fn optimize_when_load_const_zero_add_f32_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f32(0));
    bytecode.push(opcode::ADD_F32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F32(0.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_const_zero_add_f64_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f64(0));
    bytecode.push(opcode::ADD_F64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F64(0.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_const_zero_sub_f32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f32(0));
    bytecode.push(opcode::SUB_F32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F32(0.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_zero_sub_f64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f64(0));
    bytecode.push(opcode::SUB_F64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F64(0.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_nonzero_add_i32_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(42)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
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
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_div_i32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::DIV_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(1)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_i64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i64(0));
    bytecode.push(opcode::MUL_I64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I64(1)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_f32_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f32(0));
    bytecode.push(opcode::MUL_F32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F32(1.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_one_mul_f64_then_removes_both() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_f64(0));
    bytecode.push(opcode::MUL_F64);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::F64(1.0)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

#[test]
fn optimize_when_load_const_nonone_mul_i32_then_no_change() {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::MUL_I32);
    bytecode.push(opcode::RET_VOID);

    let mut constants = vec![PoolConstant::I32(5)];
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, bytecode);
}

// --- Jump safety ---
//
// The offsets a function's branches target are an input to the optimizer,
// not something it recovers from the bytes. These tests supply them
// directly; the pairing with what the emitter actually reports is covered
// under "Jump patching after optimization" below.

#[test]
fn optimize_when_jump_target_then_skips_optimization() {
    // JMP forward past a LOAD_VAR, where the STORE_VAR is the jump target.
    // The pair must NOT be optimized because STORE_VAR is targeted.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&jmp(0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched_targeting(&bytecode, &[6]), &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_jump_target_follows_removed_instructions_then_maps_to_new_position() {
    // Layout:
    //   [0] JMP              -> targets offset 10 (RET_VOID)
    //   [3] LOAD_VAR_I32 5   ]
    //   [6] STORE_VAR_I32 5  ]-- removable pair
    //   [9] LOAD_TRUE
    //   [10] RET_VOID        <- jump target
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&jmp(0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.push(opcode::RET_VOID);

    let (result, offset_map) = optimize(unpatched_targeting(&bytecode, &[10]), &mut vec![]);

    // Six bytes removed, so the target moves from 10 to 4:
    //   [0] JMP
    //   [3] LOAD_TRUE
    //   [4] RET_VOID
    let mut expected = Vec::new();
    expected.extend_from_slice(&jmp(0));
    expected.push(opcode::LOAD_TRUE);
    expected.push(opcode::RET_VOID);

    assert_eq!(result, expected);
    assert_eq!(offset_map[&10], 4, "the emitter patches against this");
}

#[test]
fn optimize_when_cmp_br_targets_removable_pair_then_pair_is_kept() {
    // The branch target must be protected from removal, otherwise the
    // branch would land on whatever instruction followed the pair.
    //
    // Layout:
    //   [0]  LOAD_TRUE
    //   [1]  CMP_BR_I32       -> targets offset 9 (the LOAD_VAR_I32)
    //   [9]  LOAD_VAR_I32 5   ]
    //   [12] STORE_VAR_I32 5  ]-- removable, but [9] is a branch target
    //   [15] RET_VOID
    let mut bytecode = Vec::new();
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.extend_from_slice(&cmp_br_i32(1, 2, 0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched_targeting(&bytecode, &[9]), &mut vec![]);

    assert_eq!(result, bytecode, "branch target must not be removed");
}

#[test]
fn optimize_when_jump_targets_end_of_function_then_maps_to_new_length() {
    // A label bound one past the last instruction — the shape an IF with no
    // ELSE produces. The map has to carry it, because there is no
    // instruction there to look up.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&jmp(0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));

    let (result, offset_map) = optimize(unpatched_targeting(&bytecode, &[9]), &mut vec![]);

    assert_eq!(result, jmp(0));
    assert_eq!(offset_map[&9], 3);
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
    let (result, _) = optimize(unpatched(&bytecode), &mut constants);
    assert_eq!(result, vec![opcode::RET_VOID]);
}

// --- String opcode regression tests (instruction size correctness) ---

#[test]
fn optimize_when_str_load_var_before_jump_then_no_panic() {
    // STR_LOAD_VAR uses a u32 operand (5 bytes total). A wrong
    // instruction size would desynchronize the decoder, and the jump
    // target would then miss the offset map and panic.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&str_load_var(100));
    bytecode.push(opcode::POP);
    bytecode.extend_from_slice(&jmp(0));
    bytecode.push(opcode::POP);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched_targeting(&bytecode, &[10]), &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_find_str_before_jump_then_no_panic() {
    // FIND_STR uses two u32 operands (9 bytes total). A wrong
    // instruction size would desynchronize the decoder.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&find_str(100, 200));
    bytecode.push(opcode::POP);
    bytecode.extend_from_slice(&jmp(0));
    bytecode.push(opcode::POP);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched_targeting(&bytecode, &[14]), &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_str_init_before_jump_then_no_panic() {
    // STR_INIT uses u32 + u16 + u8 operands (8 bytes total). A wrong
    // instruction size would desynchronize the decoder.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&str_init(100, 80));
    bytecode.extend_from_slice(&jmp(0));
    bytecode.push(opcode::POP);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched_targeting(&bytecode, &[12]), &mut vec![]);
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

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
    assert_eq!(result, bytecode);
}

#[test]
fn optimize_when_load_const_out_of_bounds_mul_i32_then_keeps_instructions() {
    // Drives is_one_constant's `_ => false` default arm.
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&load_const_i32(5));
    bytecode.push(opcode::MUL_I32);
    bytecode.push(opcode::RET_VOID);

    let (result, _) = optimize(unpatched(&bytecode), &mut vec![]);
    assert_eq!(result, bytecode);
}

// --- Offset-map composition across passes ---
//
// Each pass produces its own old->new map and the driver folds them
// together. These pin the fold, which no single-pass test can reach:
// the shared bytecode below has one pair removed by pass_self_assign
// and a second pair removed by pass_arith_identity.

/// Layout:
///   [0]  JMP              -> targets offset 14 (RET_VOID)
///   [3]  LOAD_VAR_I32 5   ]-- removed by pass_self_assign
///   [6]  STORE_VAR_I32 5  ]
///   [9]  LOAD_CONST_I32 0 ]-- removed by pass_arith_identity
///   [12] ADD_I32          ]
///   [13] LOAD_TRUE
///   [14] RET_VOID         <- jump target
fn two_pass_bytecode() -> Vec<u8> {
    let mut bytecode = Vec::new();
    bytecode.extend_from_slice(&jmp(0));
    bytecode.extend_from_slice(&load_var_i32(5));
    bytecode.extend_from_slice(&store_var_i32(5));
    bytecode.extend_from_slice(&load_const_i32(0));
    bytecode.push(opcode::ADD_I32);
    bytecode.push(opcode::LOAD_TRUE);
    bytecode.push(opcode::RET_VOID);
    bytecode
}

/// Runs [`optimize`] over [`two_pass_bytecode`] with its one jump target.
fn optimize_two_pass_bytecode(bytecode: &[u8]) -> (Vec<u8>, OffsetMap) {
    optimize(
        unpatched_targeting(bytecode, &[14]),
        &mut vec![PoolConstant::I32(0)],
    )
}

#[test]
fn optimize_when_target_spans_removals_from_two_passes_then_map_composes() {
    let (result, offset_map) = optimize_two_pass_bytecode(&two_pass_bytecode());

    // Ten bytes removed across the two passes, so the target moves from
    // 14 to 4:
    //   [0] JMP
    //   [3] LOAD_TRUE
    //   [4] RET_VOID
    let mut expected = Vec::new();
    expected.extend_from_slice(&jmp(0));
    expected.push(opcode::LOAD_TRUE);
    expected.push(opcode::RET_VOID);

    assert_eq!(result, expected);
    assert_eq!(
        offset_map[&14], 4,
        "the target must be carried through both passes' maps"
    );
}

#[test]
fn remap_line_map_when_entry_removed_by_first_pass_then_snaps_past_second_pass() {
    let bytecode = two_pass_bytecode();
    let (result, offset_map) = optimize_two_pass_bytecode(&bytecode);

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
    let (result, offset_map) = optimize_two_pass_bytecode(&bytecode);

    let raw = vec![line_entry(4, 10)];
    let diagnostic = remap_line_map(raw, &offset_map, result.len() as u16).unwrap_err();

    assert_eq!(diagnostic.code, "P9998");
    assert!(diagnostic
        .primary
        .message
        .contains("not an instruction boundary"));
}

// --- Jump patching after optimization ---
//
// The optimizer never writes a branch offset; the emitter resolves every
// jump once the optimizer has said where each instruction moved to. These
// run both halves — which is what `finalize_function` does — and assert the
// bytes that reach the container.

/// The `cmp_op` byte these tests branch on. Which comparison it is does not
/// matter; only that the instruction carries a branch offset.
const CMP_OP: u8 = opcode::cmp_op::LE_S;

#[test]
fn optimize_and_patch_when_jump_target_then_pair_is_kept_and_offset_unchanged() {
    // JMP forward past a LOAD_VAR onto the STORE_VAR, which therefore
    // survives with its partner.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.emit_jmp(label);
    em.emit_load_var_i32(VarIndex::new(5));
    em.bind_label(label);
    em.emit_store_var_i32(VarIndex::new(5));
    em.emit_ret_void();

    let mut expected = Vec::new();
    expected.extend_from_slice(&jmp(3));
    expected.extend_from_slice(&load_var_i32(5));
    expected.extend_from_slice(&store_var_i32(5));
    expected.push(opcode::RET_VOID);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), expected);
}

#[test]
fn optimize_and_patch_when_jump_over_removed_instructions_then_offset_shrinks() {
    // JMP forward over a removable pair onto the RET_VOID.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.emit_jmp(label);
    em.emit_load_var_i32(VarIndex::new(5));
    em.emit_store_var_i32(VarIndex::new(5));
    em.emit_load_true();
    em.bind_label(label);
    em.emit_ret_void();

    // Six bytes removed, so the +7 the emitter would have written becomes
    // +1:
    //   [0] JMP +1
    //   [3] LOAD_TRUE
    //   [4] RET_VOID
    let mut expected = Vec::new();
    expected.extend_from_slice(&jmp(1));
    expected.push(opcode::LOAD_TRUE);
    expected.push(opcode::RET_VOID);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), expected);
}

#[test]
fn optimize_and_patch_when_cmp_br_over_removed_instructions_then_offset_shrinks() {
    // The branch offset of a CMP_BR sits six bytes into the instruction
    // rather than one. Before jumps were patched after optimization, that
    // was the optimizer's business to know, and a stale offset landed the
    // branch inside an instruction to decode as garbage at run time.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.emit_cmp_br_i32(CMP_OP, VarIndex::new(1), 2, label);
    em.emit_load_var_i32(VarIndex::new(5));
    em.emit_store_var_i32(VarIndex::new(5));
    em.emit_load_true();
    em.bind_label(label);
    em.emit_ret_void();

    //   [0] CMP_BR_I32 +1
    //   [8] LOAD_TRUE
    //   [9] RET_VOID
    let mut expected = Vec::new();
    expected.extend_from_slice(&cmp_br_i32(1, 2, 1));
    expected.push(opcode::LOAD_TRUE);
    expected.push(opcode::RET_VOID);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), expected);
}

#[test]
fn optimize_and_patch_when_cmp_br_branches_backward_then_offset_shrinks() {
    // The loop shape: a removable pair inside the body, with the branch
    // jumping backwards over it.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.bind_label(label);
    em.emit_load_true();
    em.emit_load_var_i32(VarIndex::new(5));
    em.emit_store_var_i32(VarIndex::new(5));
    em.emit_cmp_br_i32(CMP_OP, VarIndex::new(1), 2, label);
    em.emit_ret_void();

    //   [0] LOAD_TRUE       <- branch target
    //   [1] CMP_BR_I32 -9
    //   [9] RET_VOID
    let mut expected = Vec::new();
    expected.push(opcode::LOAD_TRUE);
    expected.extend_from_slice(&cmp_br_i32(1, 2, -9));
    expected.push(opcode::RET_VOID);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), expected);
}

#[test]
fn optimize_and_patch_when_jump_targets_end_of_function_then_offset_is_zero() {
    // A label bound one past the last instruction, where there is no
    // instruction for the map to carry the position of.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.emit_jmp(label);
    em.emit_load_var_i32(VarIndex::new(5));
    em.emit_store_var_i32(VarIndex::new(5));
    em.bind_label(label);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), jmp(0));
}

#[test]
fn optimize_and_patch_when_nothing_is_removed_then_offsets_are_unchanged() {
    // Nothing matches a pass here, so this pins that running the optimizer
    // first is not itself capable of moving a branch.
    let mut em = Emitter::new();
    let label = em.create_label();
    em.emit_load_true();
    em.emit_jmp_if_not(label);
    em.emit_load_var_i32(VarIndex::new(5));
    em.emit_store_var_i32(VarIndex::new(6));
    em.bind_label(label);
    em.emit_ret_void();

    let mut expected = vec![opcode::LOAD_TRUE, opcode::JMP_IF_NOT];
    expected.extend_from_slice(&6i16.to_le_bytes());
    expected.extend_from_slice(&load_var_i32(5));
    expected.extend_from_slice(&store_var_i32(6));
    expected.push(opcode::RET_VOID);

    assert_eq!(optimize_and_patch(&mut em, &mut vec![]), expected);
}
