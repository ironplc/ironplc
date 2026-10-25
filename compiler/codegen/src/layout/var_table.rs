//! The limit of the variable table. The container counts its slots by a
//! `u16` (`num_variables`), so it holds at most 65,535 of them, numbered `0`
//! to `65,534`; `0xFFFF` is [`VarIndex::NO_SINGLE_VAR`], which names none.
//!
//! The limit is a fixed property of the bytecode format, so a program that
//! needs more slots reports P9997 (`NotSupported`), rather than numbering
//! them modulo 65,536 and writing a container whose variables alias each
//! other (REQ-CF-codegen-035). The layout counts its slots with a
//! [`Counter`], which saturates at the limit and remembers that the table
//! was full.

use ironplc_container::VarIndex;
use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

/// The most slots the variable table holds.
const MAX_SLOTS: u16 = u16::MAX;

/// Whether every slot the layout handed out, and every run it stepped past,
/// fits the variable table.
#[derive(Debug, Default)]
pub(crate) struct Counter {
    full: bool,
}

impl Counter {
    /// The slot `index`, which a variable takes; the last slot of the table,
    /// and the table marked full, when the table has no slot `index`.
    pub(crate) fn slot(&mut self, index: usize) -> VarIndex {
        match u16::try_from(index) {
            Ok(index) if index < MAX_SLOTS => VarIndex::new(index),
            _ => {
                self.full = true;
                VarIndex::new(MAX_SLOTS - 1)
            }
        }
    }

    /// The slot after `slot`, past the slot `slot` handed out; the table's
    /// count of slots, and the table marked full, when `slot` is the table's
    /// count, which is not one of its slots.
    pub(crate) fn after(&mut self, slot: VarIndex) -> VarIndex {
        if slot.raw() == MAX_SLOTS {
            self.full = true;
            return VarIndex::new(MAX_SLOTS);
        }
        VarIndex::new(slot.raw() + 1)
    }

    /// P9997 at `span` when a slot did not fit the table.
    #[track_caller]
    pub(crate) fn check(&self, span: &SourceSpan) -> Result<(), Diagnostic> {
        if !self.full {
            return Ok(());
        }
        Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            format!("Variable table exceeds {MAX_SLOTS} slots"),
        )))
    }
}

#[cfg(test)]
mod tests {
    use ironplc_container::VarIndex;
    use ironplc_ir::SourceSpan;

    use super::{Counter, MAX_SLOTS};

    #[test]
    fn slot_when_index_is_last_slot_then_fits() {
        let mut counter = Counter::default();

        let slot = counter.slot(usize::from(MAX_SLOTS - 1));

        assert_eq!(slot, VarIndex::new(MAX_SLOTS - 1));
        assert!(counter.check(&SourceSpan::default()).is_ok());
    }

    #[test]
    fn slot_when_index_is_table_count_then_p9997() {
        let mut counter = Counter::default();

        counter.slot(usize::from(MAX_SLOTS));

        let err = counter.check(&SourceSpan::default()).unwrap_err();
        assert_eq!(err.code, "P9997");
    }

    #[test]
    fn slot_when_index_does_not_fit_u16_then_p9997() {
        let mut counter = Counter::default();

        counter.slot(usize::from(u16::MAX) + 1);

        let err = counter.check(&SourceSpan::default()).unwrap_err();
        assert_eq!(err.code, "P9997");
    }

    #[test]
    fn after_when_slot_is_last_slot_then_table_count() {
        let mut counter = Counter::default();

        let next = counter.after(VarIndex::new(MAX_SLOTS - 1));

        assert_eq!(next, VarIndex::new(MAX_SLOTS));
        assert!(counter.check(&SourceSpan::default()).is_ok());
    }

    #[test]
    fn after_when_slot_is_past_table_then_p9997() {
        let mut counter = Counter::default();

        let next = counter.after(VarIndex::new(MAX_SLOTS));

        assert_eq!(next, VarIndex::new(MAX_SLOTS));
        let err = counter.check(&SourceSpan::default()).unwrap_err();
        assert_eq!(err.code, "P9997");
    }
}
