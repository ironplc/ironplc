//! Shared fixtures and helpers for tool unit tests.
//!
//! The compile pipeline itself (parse, analyze, codegen) is owned and tested
//! by the parser, analyzer, and codegen crates, and source-name/options
//! validation is owned by `tools::common`. Per-tool tests use these fixtures
//! to prove the tool wires that shared infrastructure in — one wiring test
//! per concern — plus whatever response shape is specific to the tool.
//!
//! The source snippets themselves live in [`ironplc_test::fixtures`] so the
//! integration tests under `tests/`, which cannot see this `#[cfg(test)]`
//! module, share the same text; this module re-exports them and adds the
//! wrappers that need this crate's types.

use super::common::SourceInput;
use serde_json::{json, Value};

pub use ironplc_test::fixtures::*;

/// Options selecting the IEC 61131-3 second-edition dialect.
pub fn ed2_options() -> Value {
    json!({"dialect": "iec61131-3-ed2"})
}

/// `prelude` followed by [`VALID_PROGRAM`], for a test whose subject is a
/// top-level declaration (or comment) that needs a program alongside it.
pub fn with_program(prelude: &str) -> String {
    format!("{prelude}\n{VALID_PROGRAM}")
}

/// A single source named `main.st` with the given content.
pub fn source(content: &str) -> Vec<SourceInput> {
    vec![SourceInput {
        name: "main.st".into(),
        content: content.into(),
    }]
}

/// A single source with an invalid (empty) name, rejected by
/// `common::validate_sources` before any tool logic runs.
pub fn unnamed_source() -> Vec<SourceInput> {
    vec![SourceInput {
        name: String::new(),
        content: VALID_PROGRAM.into(),
    }]
}
