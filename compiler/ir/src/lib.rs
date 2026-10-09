//! IronPLC's target-neutral intermediate representation.
//!
//! A backend reads what this crate holds and decides nothing about what the
//! language means. It holds the execution model today ([`execution`]); see
//! `specs/design/execution-model.md`.

pub mod execution;

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
