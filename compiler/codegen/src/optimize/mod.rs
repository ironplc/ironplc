//! Post-emission peephole optimizer for bytecode.
//!
//! Runs an ordered sequence of passes over the raw bytecode buffer produced by
//! the emitter. Each pass matches adjacent instruction pairs against one family
//! of identity/no-op patterns and removes them; jump offsets are adjusted to
//! account for removed bytes.
//!
//! These passes run after the emitter's own in-line peephole optimizations
//! (consecutive load -> DUP, store-load -> DUP+STORE) and complement them by
//! handling patterns that are only visible once the full instruction stream
//! exists.
//!
//! Passes:
//!
//! 1. [`pass_self_assign`] — `LOAD_VAR x; STORE_VAR x` (same var, same type).
//! 2. [`pass_arith_identity`] — `LOAD_CONST 0; ADD|SUB` and
//!    `LOAD_CONST 1; MUL|DIV` (matching width).
//!
//! Instructions that are the target of a jump are never removed; this
//! preserves basic-block boundaries and guarantees jump targets always
//! map to a valid new offset.

mod pass_arith_identity;
mod pass_self_assign;
mod rewrite;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::compile::PoolConstant;

/// Maps every old (pre-optimization) byte offset to its new offset in the
/// optimized bytecode. Includes one-past-the-end so spans that touch the end
/// of the function can still be remapped.
pub(crate) type OffsetMap = HashMap<usize, usize>;

/// Remaps an emitter line map through the optimizer's old→new offset table.
///
/// For each entry the old `bytecode_offset` is looked up in `offset_map`,
/// which by construction maps every instruction boundary — including
/// removed instructions — to the position the next surviving instruction
/// occupies in the optimized stream ("snap forward"). Entries whose
/// remapped offset lands past the new end-of-function (`new_bytecode_len`)
/// are dropped, and entries that map to the same remapped offset as the
/// previous kept entry are deduplicated (the optimizer can collapse two
/// pre-optimization positions onto one post-optimization position).
///
/// Entries whose old offset is not in the map are dropped silently; the
/// emitter only records entries immediately before pushing an opcode,
/// so this should never happen in practice. If it does, the resulting
/// debug info would be wrong anyway, so silently dropping is safer than
/// keeping a bad offset.
pub(crate) fn remap_line_map(
    raw: Vec<crate::emit::EmittedLineMapEntry>,
    offset_map: &OffsetMap,
    new_bytecode_len: u16,
) -> Vec<crate::emit::EmittedLineMapEntry> {
    let mut out: Vec<crate::emit::EmittedLineMapEntry> = Vec::with_capacity(raw.len());
    for entry in raw {
        let Some(&new_offset) = offset_map.get(&(entry.bytecode_offset as usize)) else {
            continue;
        };
        if new_offset >= new_bytecode_len as usize {
            continue;
        }
        let new_offset = new_offset as u16;
        if out.last().is_some_and(|e| e.bytecode_offset == new_offset) {
            continue;
        }
        out.push(crate::emit::EmittedLineMapEntry {
            bytecode_offset: new_offset,
            ..entry
        });
    }
    out
}

/// Threads bytecode through the pass sequence, accumulating one old→new
/// offset map that covers the whole pipeline.
struct Pipeline {
    bytecode: Vec<u8>,
    /// Maps original offsets to offsets in `bytecode`. `None` until the first
    /// pass has run, after which it is that pass's own map.
    map: Option<OffsetMap>,
}

impl Pipeline {
    fn run(&mut self, pass: impl FnOnce(&[u8]) -> (Vec<u8>, OffsetMap)) {
        let (bytecode, map) = pass(&self.bytecode);
        self.bytecode = bytecode;
        // Composition is total: every value in the accumulated map is an
        // offset this pass's input had an instruction boundary at (or its
        // length), because both are accumulated from surviving instruction
        // sizes — and those are exactly the keys `map` covers.
        self.map = Some(match self.map.take() {
            None => map,
            Some(prev) => prev
                .into_iter()
                .map(|(old, intermediate)| (old, map[&intermediate]))
                .collect(),
        });
    }
}

/// Runs the peephole optimizer on `bytecode`.
///
/// Returns the optimized byte vector along with an old→new offset map. The
/// offset map covers every original instruction's start offset plus the
/// one-past-the-end position, so callers can remap any span that points into
/// (or just past) the original bytecode. If no patterns are found, the
/// output bytes equal the input and the map is the identity over instruction
/// boundaries.
pub(crate) fn optimize(bytecode: &[u8], constants: &[PoolConstant]) -> (Vec<u8>, OffsetMap) {
    if bytecode.is_empty() {
        return (Vec::new(), OffsetMap::new());
    }

    let mut pipeline = Pipeline {
        bytecode: bytecode.to_vec(),
        map: None,
    };

    // The passes below match on disjoint opcode pairs, so this order does not
    // affect the result. It is fixed and named here so that a pass whose
    // output feeds another has one obvious place to say so — and one obvious
    // place to add a fixed-point loop, if one is ever needed. Nothing needs
    // one today.
    pipeline.run(pass_self_assign::apply);
    pipeline.run(|bytecode| pass_arith_identity::apply(bytecode, constants));

    (
        pipeline.bytecode,
        pipeline.map.expect("at least one pass runs"),
    )
}
