//! Shared machinery for the peephole passes.
//!
//! Every pass expresses itself as a decision about two adjacent instructions;
//! everything around that decision — decoding, protecting jump targets,
//! rebuilding the byte stream, rewriting branch offsets and recording the
//! old→new offset map — is written once here.

use std::collections::HashSet;

use ironplc_container::opcode;

use super::OffsetMap;

/// A decoded instruction: its original byte offset and raw bytes.
pub(super) struct Instruction {
    pub(super) offset: usize,
    pub(super) bytes: Vec<u8>,
}

impl Instruction {
    pub(super) fn opcode(&self) -> u8 {
        self.bytes[0]
    }

    pub(super) fn u16_operand(&self) -> u16 {
        u16::from_le_bytes([self.bytes[1], self.bytes[2]])
    }
}

/// Encoded size of a `CMP_BR_*` instruction: opcode + u8 + u16 + u16 + i16.
/// The branch offset occupies the trailing `i16` and is relative to the end
/// of the instruction.
const CMP_BR_SIZE: usize = 8;

/// Returns true for the compare-and-branch superinstructions, which carry a
/// relative branch offset like `JMP`/`JMP_IF_NOT` but in a different slot.
fn is_cmp_br(op: u8) -> bool {
    op == opcode::CMP_BR_I32 || op == opcode::CMP_BR_I64
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
        // CMP_BR is a branch too: its target must be protected from removal
        // and its offset rewritten, exactly like JMP/JMP_IF_NOT.
        if is_cmp_br(op) && end - pc >= CMP_BR_SIZE {
            let rel = i16::from_le_bytes([bytecode[pc + 6], bytecode[pc + 7]]);
            let target = (pc as isize + CMP_BR_SIZE as isize + rel as isize) as usize;
            jump_targets.insert(target);
        }

        pc = end;
    }

    (instructions, jump_targets)
}

/// Runs one peephole pass over `bytecode`.
///
/// `matches` is asked about each adjacent instruction pair in turn; returning
/// true removes both. Instructions that are the target of a jump are never
/// offered to `matches` and so are never removed; this preserves basic-block
/// boundaries and guarantees jump targets always map to a valid new offset.
///
/// Returns the rewritten bytes along with an old→new offset map. The map
/// covers every instruction's start offset plus the one-past-the-end position,
/// so callers can remap any span that points into (or just past) the input.
/// Removed instructions map to the position the next surviving instruction
/// occupies, so a span that lands on one snaps forward rather than dangling.
pub(super) fn apply_peephole(
    bytecode: &[u8],
    mut matches: impl FnMut(&Instruction, &Instruction) -> bool,
) -> (Vec<u8>, OffsetMap) {
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

        if matches(a, b) {
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
    let mut offset_map: OffsetMap = OffsetMap::new();
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
        } else if is_cmp_br(op) && instr.bytes.len() == CMP_BR_SIZE {
            let old_rel = i16::from_le_bytes([instr.bytes[6], instr.bytes[7]]);
            let old_target =
                (instr.offset as isize + CMP_BR_SIZE as isize + old_rel as isize) as usize;
            let new_pos = output.len();
            let new_target = offset_map[&old_target];
            let new_rel = (new_target as isize - (new_pos as isize + CMP_BR_SIZE as isize)) as i16;
            output.extend_from_slice(&instr.bytes[..6]);
            output.extend_from_slice(&new_rel.to_le_bytes());
        } else {
            output.extend_from_slice(&instr.bytes);
        }
    }

    (output, offset_map)
}
