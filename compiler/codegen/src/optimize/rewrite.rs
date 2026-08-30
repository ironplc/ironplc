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

/// What [`apply_peephole`] does with one instruction of a matched pair.
pub(super) enum Action {
    /// Leave the instruction exactly as it is.
    Keep,
    /// Drop the instruction from the output.
    Remove,
    /// Keep the instruction but replace its `u16` operand. Only valid for a
    /// 3-byte opcode-plus-`u16` instruction, so the encoded size is unchanged
    /// and offsets are unaffected.
    RewriteOperand(u16),
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

/// Decode raw bytecode into a list of instructions.
fn decode(bytecode: &[u8]) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        let size = opcode::instruction_size(op);
        let end = (pc + size).min(bytecode.len());
        instructions.push(Instruction {
            offset: pc,
            bytes: bytecode[pc..end].to_vec(),
        });
        pc = end;
    }

    instructions
}

/// Recovers the offsets branch instructions in `bytecode` target.
///
/// The branch offset is relative to the end of the instruction carrying it,
/// so the target is recovered by adding the instruction's size back in.
pub(super) fn jump_targets(bytecode: &[u8]) -> HashSet<usize> {
    let mut targets = HashSet::new();
    let mut pc = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        let end = (pc + opcode::instruction_size(op)).min(bytecode.len());

        if (op == opcode::JMP || op == opcode::JMP_IF_NOT) && end - pc >= 3 {
            let rel = i16::from_le_bytes([bytecode[pc + 1], bytecode[pc + 2]]);
            targets.insert((pc as isize + 3 + rel as isize) as usize);
        }
        // CMP_BR is a branch too: its target must be protected from removal
        // and its offset rewritten, exactly like JMP/JMP_IF_NOT.
        if is_cmp_br(op) && end - pc >= CMP_BR_SIZE {
            let rel = i16::from_le_bytes([bytecode[pc + 6], bytecode[pc + 7]]);
            targets.insert((pc as isize + CMP_BR_SIZE as isize + rel as isize) as usize);
        }

        pc = end;
    }

    targets
}

/// Runs one peephole pass over `bytecode`.
///
/// `matches` is asked about each adjacent instruction pair in turn. Returning
/// `Some` applies one [`Action`] to each instruction of the pair; returning
/// `None` leaves both alone. Instructions starting at an offset in
/// `protected` are never offered to `matches` and so are never rewritten;
/// callers put every jump target there, which preserves basic-block
/// boundaries and guarantees jump targets always map to a valid new offset.
///
/// Returns the rewritten bytes along with an old→new offset map. The map
/// covers every instruction's start offset plus the one-past-the-end position,
/// so callers can remap any span that points into (or just past) the input.
/// Removed instructions map to the position the next surviving instruction
/// occupies, so a span that lands on one snaps forward rather than dangling.
pub(super) fn apply_peephole(
    bytecode: &[u8],
    protected: &HashSet<usize>,
    mut matches: impl FnMut(&Instruction, &Instruction) -> Option<[Action; 2]>,
) -> (Vec<u8>, OffsetMap) {
    let instructions = decode(bytecode);

    // First pass: decide what happens to each instruction of a matched pair.
    let mut actions: Vec<Action> = instructions.iter().map(|_| Action::Keep).collect();
    let mut i = 0;
    while i + 1 < instructions.len() {
        let a = &instructions[i];
        let b = &instructions[i + 1];

        // Never touch instructions that are the target of a jump.
        if protected.contains(&a.offset) || protected.contains(&b.offset) {
            i += 1;
            continue;
        }

        if let Some([first, second]) = matches(a, b) {
            actions[i] = first;
            actions[i + 1] = second;
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
        // `RewriteOperand` preserves the encoded size, so only `Remove`
        // changes where the following instructions land.
        if !matches!(actions[idx], Action::Remove) {
            new_offset += instr.bytes.len();
        }
    }
    offset_map.insert(bytecode.len(), new_offset);

    // Second pass: rebuild bytecode, rewriting jump offsets.
    let mut output = Vec::with_capacity(bytecode.len());
    for (idx, instr) in instructions.iter().enumerate() {
        let op = instr.opcode();
        match actions[idx] {
            Action::Remove => continue,
            Action::RewriteOperand(operand) => {
                debug_assert_eq!(
                    instr.bytes.len(),
                    3,
                    "RewriteOperand requires an opcode plus u16 operand"
                );
                output.push(op);
                output.extend_from_slice(&operand.to_le_bytes());
                continue;
            }
            Action::Keep => {}
        }
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
