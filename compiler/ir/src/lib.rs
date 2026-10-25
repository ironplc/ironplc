//! IronPLC's target-neutral intermediate representation.
//!
//! A backend reads what this crate holds and decides nothing about what the
//! language means. It holds the execution model ([`execution`]; see
//! `specs/design/execution-model.md`), the lowered program ([`lowered`];
//! see `specs/design/lowered-program.md`), and how a backend reads the
//! source bytes its debug information hashes ([`source`]).

// A match over a lowered enum names every variant, so a variant added later
// fails to compile wherever it is not handled (REQ-LOW-lowering-082).
#![deny(clippy::wildcard_enum_match_arm)]
#![allow(clippy::result_large_err)]

pub mod execution;
pub mod lowered;
pub mod source;

// What a backend needs from `ironplc-dsl` to label a diagnostic and find a
// source file, re-exported so that a backend names them through this crate
// and needs no dependency on `ironplc-dsl` (`specs/design/lowered-program.md`,
// "Position in the Pipeline").
pub use ironplc_dsl::core::{FileId, Id, Located, SourceSpan};
pub use ironplc_dsl::diagnostic::{Diagnostic, Label, LineColumn};

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
