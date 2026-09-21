//! Control-flow scaffolding shared by the bytecode verification passes.
//!
//! A verification pass that answers a question about *every path* through a
//! function body — rather than about one instruction in isolation — needs
//! the same three things: which byte offsets start an instruction, where
//! control goes after each one, and whether a branch operand resolves to a
//! real instruction. None of that depends on what the pass is tracking, so
//! it lives here and each pass supplies only its own abstract state.
//!
//! The errors this module reports carry no `FunctionId`. A pass verifies one
//! function at a time and already knows which; it adds that when mapping a
//! [`CfgError`] into its own error type, so the same scaffolding serves
//! passes whose error enums are otherwise unrelated.

use std::vec;
use std::vec::Vec;

use crate::opcode::{self, DecodeStop, Opcode};

/// A body that cannot be walked at all.
///
/// Every variant means the same thing to a caller: the bytecode does not
/// decode into a control-flow graph, so no path-based claim about it can be
/// made. A pass turns this into whichever of its own variants says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CfgError {
    /// A byte that is not an assigned opcode.
    UnknownOpcode { offset: usize, byte: u8 },
    /// An instruction whose operands run past the end of the body.
    TruncatedInstruction { offset: usize, opcode: Opcode },
    /// A branch whose target is outside the body, or lands inside an
    /// instruction rather than on its first byte.
    InvalidJumpTarget { offset: usize, target: isize },
}

/// Where control can go after an instruction.
///
/// [`Return`](Flow::Return) carries no payload: what a path owes when it
/// leaves the function is the leaving pass's business, and the opcode that
/// produced the flow is still in the caller's hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flow {
    /// Continue at the next instruction.
    Next,
    /// Continue only at the branch target.
    Jump(isize),
    /// Continue at either the branch target or the next instruction.
    Branch(isize),
    /// Leave the function.
    Return,
}

/// Marks every byte offset that starts an instruction, by decoding the
/// body linearly from offset 0.
///
/// Unlike the passes that only look for a pattern, this one rejects the
/// container over any byte it cannot read: an offset this walk does not mark
/// is one an abstract interpretation would refuse to branch to, so a body
/// that does not decode cleanly cannot be verified at all.
pub(crate) fn instruction_boundaries(bytecode: &[u8]) -> Result<Vec<bool>, CfgError> {
    let mut boundaries = vec![false; bytecode.len()];
    for decoded in opcode::decode_body(bytecode) {
        match decoded {
            Ok(instruction) => boundaries[instruction.offset] = true,
            Err(DecodeStop::UnknownOpcode { offset, byte }) => {
                return Err(CfgError::UnknownOpcode { offset, byte })
            }
            Err(DecodeStop::Truncated { offset, opcode }) => {
                return Err(CfgError::TruncatedInstruction { offset, opcode })
            }
        }
    }
    Ok(boundaries)
}

/// Resolves a branch operand to an absolute offset, rejecting a target
/// outside the body or inside an instruction.
pub(crate) fn branch_target(
    pc: usize,
    size: usize,
    relative: isize,
    boundaries: &[bool],
    len: usize,
) -> Result<usize, CfgError> {
    let target = pc as isize + size as isize + relative;
    let invalid = CfgError::InvalidJumpTarget { offset: pc, target };
    if target < 0 || target as usize > len {
        return Err(invalid);
    }
    let target = target as usize;
    // `len` (one past the end) is a legal target: it falls off the body,
    // which the VM treats as RET_VOID.
    if target < len && !boundaries[target] {
        return Err(invalid);
    }
    Ok(target)
}

/// Control-flow successors of an instruction.
///
/// Only branch and return opcodes deviate from straight-line flow, so this
/// match lists them explicitly and everything else falls through.
pub(crate) fn flow_of(op: Opcode, operands: &[u8]) -> Flow {
    match op {
        opcode::JMP => Flow::Jump(i16_at(operands, 0) as isize),
        opcode::JMP_IF_NOT => Flow::Branch(i16_at(operands, 0) as isize),
        // CMP_BR: [cmp_op u8][var u16][const u16][target i16]
        opcode::CMP_BR_I32 | opcode::CMP_BR_I64 => Flow::Branch(i16_at(operands, 5) as isize),
        opcode::RET | opcode::RET_VOID => Flow::Return,
        _ => Flow::Next,
    }
}

/// Reads a little-endian `u16` from `operands` at `index`.
pub(crate) fn u16_at(operands: &[u8], index: usize) -> u16 {
    u16::from_le_bytes([operands[index], operands[index + 1]])
}

/// Reads a little-endian `i16` from `operands` at `index`.
pub(crate) fn i16_at(operands: &[u8], index: usize) -> i16 {
    i16::from_le_bytes([operands[index], operands[index + 1]])
}
