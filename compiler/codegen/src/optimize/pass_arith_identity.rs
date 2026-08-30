//! Removes arithmetic identity operations against a constant operand:
//!
//! - `LOAD_CONST 0; ADD|SUB` (matching width) — additive identity
//! - `LOAD_CONST 1; MUL|DIV` (matching width) — multiplicative identity
//!
//! Both instructions are removed, leaving the other operand on the stack.
//! These are only visible once the full instruction stream exists, which is
//! why the emitter's own in-line peepholes cannot catch them.

use ironplc_container::opcode;

use super::rewrite::{apply_peephole, Instruction};
use super::OffsetMap;
use crate::compile::PoolConstant;

pub(super) fn apply(bytecode: &[u8], constants: &[PoolConstant]) -> (Vec<u8>, OffsetMap) {
    apply_peephole(bytecode, |a, b| is_identity(a, b, constants))
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

fn is_identity(a: &Instruction, b: &Instruction, constants: &[PoolConstant]) -> bool {
    if a.bytes.len() != 3 {
        return false;
    }
    let a_op = a.opcode();
    let b_op = b.opcode();
    let pool_idx = a.u16_operand();

    if let Some((add_op, sub_op)) = additive_ops_for_const(a_op) {
        if (b_op == add_op || b_op == sub_op) && is_zero_constant(constants, pool_idx) {
            return true;
        }
    }

    if let Some((mul_op, div_op)) = multiplicative_ops_for_const(a_op) {
        if (b_op == mul_op || b_op == div_op) && is_one_constant(constants, pool_idx) {
            return true;
        }
    }

    false
}
