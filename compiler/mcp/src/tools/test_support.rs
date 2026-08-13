//! Shared fixtures and helpers for tool unit tests.
//!
//! The compile pipeline itself (parse, analyze, codegen) is owned and tested
//! by the parser, analyzer, and codegen crates, and source-name/options
//! validation is owned by `tools::common`. Per-tool tests use these fixtures
//! to prove the tool wires that shared infrastructure in — one wiring test
//! per concern — plus whatever response shape is specific to the tool.

use super::common::SourceInput;
use serde_json::{json, Value};

/// A minimal valid program.
pub const VALID_PROGRAM: &str = "PROGRAM p\nEND_PROGRAM";

/// A program with a syntax error (unterminated declaration).
pub const SYNTAX_ERROR_PROGRAM: &str = "PROGRAM";

/// A program with a semantic error (undeclared variable `y`).
pub const SEMANTIC_ERROR_PROGRAM: &str = "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM";

/// Options selecting the IEC 61131-3 second-edition dialect.
pub fn ed2_options() -> Value {
    json!({"dialect": "iec61131-3-ed2"})
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
