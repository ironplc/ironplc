//! Removes redundant self-assignment: `LOAD_VAR x; STORE_VAR x`.
//!
//! The pair loads a variable and immediately stores it back to the same slot
//! at the same width, leaving the variable table unchanged. Both instructions
//! are removed.

use ironplc_container::opcode;

use super::rewrite::{apply_peephole, Instruction};
use super::OffsetMap;

pub(super) fn apply(bytecode: &[u8]) -> (Vec<u8>, OffsetMap) {
    apply_peephole(bytecode, is_self_assignment)
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

fn is_self_assignment(a: &Instruction, b: &Instruction) -> bool {
    let Some(expected_store) = matching_store_for_load(a.opcode()) else {
        return false;
    };
    b.opcode() == expected_store
        && a.bytes.len() == 3
        && b.bytes.len() == 3
        && a.u16_operand() == b.u16_operand()
}
