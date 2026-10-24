//! The limit of the variable table. The container counts its slots by a
//! `u16` (`num_variables`), so it holds at most 65,535 of them, numbered `0`
//! to `65,534`; `0xFFFF` is [`VarIndex::NO_SINGLE_VAR`], which names none.
//!
//! The limit is a fixed property of the bytecode format, so a program that
//! needs more slots reports P9997 (`NotSupported`) where the layout hands out
//! the slot that does not fit, rather than numbering it modulo 65,536 and
//! writing a container whose variables alias each other (REQ-CF-codegen-035).
//!
//! `#[track_caller]` keeps the compiler `file#Lline` that
//! [`Diagnostic::not_supported`] records pointing at the layout site that
//! reached the limit rather than at this module, as `data_region.rs` does.

use ironplc_container::VarIndex;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

/// The most slots the variable table holds.
pub(crate) const MAX_SLOTS: u16 = u16::MAX;

/// The slot `index` of the variable table, which the variable declared at
/// `span` takes; P9997 when the table has no slot `index`.
#[track_caller]
pub(crate) fn slot(index: usize, span: &SourceSpan) -> Result<VarIndex, Diagnostic> {
    match u16::try_from(index) {
        Ok(index) if index < MAX_SLOTS => Ok(VarIndex::new(index)),
        _ => Err(table_full(span)),
    }
}

/// The slot `by` slots on from `cursor`, past a run of `by` slots handed out
/// from it to the variables declared at `span`; P9997 when the run does not
/// fit the table.
///
/// A run fits when it ends at or before [`MAX_SLOTS`], so the slot this
/// returns is at most the table's count of slots, and a slot handed out from
/// it is checked by the next advance past it.
#[track_caller]
pub(crate) fn advance(
    cursor: VarIndex,
    by: u16,
    span: &SourceSpan,
) -> Result<VarIndex, Diagnostic> {
    match cursor.raw().checked_add(by) {
        Some(end) => Ok(VarIndex::new(end)),
        None => Err(table_full(span)),
    }
}

#[track_caller]
fn table_full(span: &SourceSpan) -> Diagnostic {
    Diagnostic::not_supported(Label::span(
        span.clone(),
        format!("Variable table exceeds {MAX_SLOTS} slots"),
    ))
}

#[cfg(test)]
mod tests {
    use ironplc_container::VarIndex;
    use ironplc_dsl::core::{Id, SourceSpan};

    use super::{advance, slot, MAX_SLOTS};
    use crate::compile::CompileContext;
    use crate::layout::names::ScopeId;

    /// A context laying out the program, which sees `count` slots.
    fn program_seeing(count: u16) -> CompileContext {
        let mut ctx = CompileContext::new();
        ctx.scope = ScopeId::Program;
        for index in 0..count {
            let name = Id::from(&format!("x{index}"));
            ctx.variables
                .insert(ScopeId::Program, name, VarIndex::new(index));
        }
        ctx
    }

    #[test]
    fn slot_when_index_is_last_slot_then_ok() {
        let index = slot(65_534, &SourceSpan::default()).unwrap();

        assert_eq!(65_534, index.raw());
    }

    #[test]
    fn slot_when_index_is_table_count_then_p9997() {
        let err = slot(usize::from(MAX_SLOTS), &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn slot_when_index_does_not_fit_u16_then_p9997() {
        let err = slot(70_000, &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn advance_when_run_ends_at_table_count_then_ok() {
        let end = advance(VarIndex::new(65_534), 1, &SourceSpan::default()).unwrap();

        assert_eq!(MAX_SLOTS, end.raw());
    }

    #[test]
    fn advance_when_run_ends_past_table_count_then_p9997() {
        let err = advance(VarIndex::new(MAX_SLOTS), 1, &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn advance_when_run_is_longer_than_room_left_then_p9997() {
        let err = advance(VarIndex::new(65_000), 600, &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn next_slot_when_program_sees_all_but_last_slot_then_last_slot() {
        let ctx = program_seeing(65_534);

        let index = ctx.next_slot(&SourceSpan::default()).unwrap();

        assert_eq!(65_534, index.raw());
    }

    #[test]
    fn next_slot_when_program_sees_every_slot_then_p9997() {
        let ctx = program_seeing(MAX_SLOTS);

        let err = ctx.next_slot(&SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn allocate_scratch_variable_when_program_sees_all_but_last_slot_then_last_slot() {
        let mut ctx = program_seeing(65_534);

        let index = ctx
            .allocate_scratch_variable("s", &SourceSpan::default())
            .unwrap();

        assert_eq!(65_534, index.raw());
    }

    #[test]
    fn allocate_scratch_variable_when_program_sees_every_slot_then_p9997() {
        let mut ctx = program_seeing(MAX_SLOTS);

        let err = ctx
            .allocate_scratch_variable("s", &SourceSpan::default())
            .unwrap_err();

        assert_eq!("P9997", err.code);
    }
}
