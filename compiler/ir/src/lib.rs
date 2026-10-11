//! IronPLC's target-neutral intermediate representation.
//!
//! A backend reads what this crate holds and decides nothing about what the
//! language means. It holds the execution model today ([`execution`]); see
//! `specs/design/execution-model.md`.

pub mod execution;

// What a backend needs from `ironplc-dsl` to label a diagnostic and find a
// source file, re-exported so that a backend names them through this crate
// and needs no dependency on `ironplc-dsl` (`specs/design/lowered-program.md`,
// "Position in the Pipeline").
pub use ironplc_dsl::core::{FileId, SourceSpan};
pub use ironplc_dsl::diagnostic::{Diagnostic, Label};

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
