//! Removes arithmetic identity operations against a constant operand:
//!
//! - `LOAD_CONST_I32|I64 0; ADD|SUB` (matching width) — additive identity
//!   on integers
//! - `LOAD_CONST_F32|F64 0.0; SUB` (matching width) — additive identity on
//!   floats, subtraction only
//! - `LOAD_CONST 1; MUL|DIV` (matching width) — multiplicative identity
//!
//! Both instructions are removed, leaving the other operand on the stack.
//! These are only visible once the full instruction stream exists, which is
//! why the emitter's own in-line peepholes cannot catch them.
//!
//! # Soundness on floats
//!
//! `x + 0.0` is deliberately *not* treated as an identity on `F32`/`F64`.
//! Under IEEE 754 (§6.3) the sum of two zeros of opposite sign is `+0.0`, so
//! `(-0.0) + 0.0 = +0.0` and removing the add would leave `-0.0` on the
//! stack. The sign of zero is observable: `1.0 / y` is `+inf` for one and
//! `-inf` for the other. `x - 0.0` is safe — `(-0.0) - 0.0 = -0.0` — as are
//! `x * 1.0` and `x / 1.0`, which preserve sign, magnitude, NaN payload and
//! infinities alike. The float `ADD` row is therefore absent from the
//! additive table below, on purpose.

use std::collections::HashSet;

use ironplc_container::opcode;

use super::rewrite::{apply_peephole, Action, Instruction};
use super::OffsetMap;
use crate::compile::PoolConstant;

pub(super) fn apply(
    bytecode: &[u8],
    protected: &HashSet<usize>,
    constants: &[PoolConstant],
) -> (Vec<u8>, OffsetMap) {
    apply_peephole(bytecode, protected, |a, b| is_identity(a, b, constants))
}

/// Returns true if the numeric constant at `pool_index` is zero.
///
/// Matches `-0.0` as well as `+0.0`: the only additive use of a float zero
/// is `SUB`, and `x - (-0.0)` is `x` for every `x` just as `x - 0.0` is.
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

/// Returns the opcodes that are an identity against a zero constant of the
/// given `LOAD_CONST` width, or None.
///
/// Integers get both `ADD` and `SUB`; floats get only `SUB` (see the module
/// doc for why `ADD` is unsound on floats).
fn additive_identity_ops_for_const(const_op: u8) -> Option<&'static [u8]> {
    match const_op {
        opcode::LOAD_CONST_I32 => Some(&[opcode::ADD_I32, opcode::SUB_I32]),
        opcode::LOAD_CONST_I64 => Some(&[opcode::ADD_I64, opcode::SUB_I64]),
        opcode::LOAD_CONST_F32 => Some(&[opcode::SUB_F32]),
        opcode::LOAD_CONST_F64 => Some(&[opcode::SUB_F64]),
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

fn is_identity(
    a: &Instruction,
    b: &Instruction,
    constants: &[PoolConstant],
) -> Option<[Action; 2]> {
    if a.bytes.len() != 3 {
        return None;
    }
    let a_op = a.opcode();
    let b_op = b.opcode();
    let pool_idx = a.u16_operand();

    if let Some(ops) = additive_identity_ops_for_const(a_op) {
        if ops.contains(&b_op) && is_zero_constant(constants, pool_idx) {
            return Some([Action::Remove, Action::Remove]);
        }
    }

    if let Some((mul_op, div_op)) = multiplicative_ops_for_const(a_op) {
        if (b_op == mul_op || b_op == div_op) && is_one_constant(constants, pool_idx) {
            return Some([Action::Remove, Action::Remove]);
        }
    }

    None
}
