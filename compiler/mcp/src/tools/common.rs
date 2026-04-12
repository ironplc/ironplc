//! Shared types and helpers for MCP tools that accept `sources` and `options`.
//!
//! Every source-accepting tool (parse, check, compile, run, …) reuses
//! the types and validation functions defined here.
//!
//! Validation errors produce standard `Diagnostic` values with problem code
//! `P8001` — the same diagnostic type the compiler uses everywhere.

use std::collections::HashSet;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ───────────────────────────────────────────────────────────────────
// Input types
// ───────────────────────────────────────────────────────────────────

/// A single source file supplied by the caller.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SourceInput {
    /// Logical file name (e.g. `"main.st"`). Must be non-empty, ≤ 256 bytes,
    /// contain only printable ASCII (0x20–0x7E), and be unique within the
    /// sources array.
    pub name: String,
    /// The full source text of the file.
    pub content: String,
}

/// Combined input accepted by `parse` and `check`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseCheckInput {
    /// One or more source files.
    pub sources: Vec<SourceInput>,
    /// Compiler options (dialect + optional feature-flag overrides).
    #[schemars(with = "serde_json::Value")]
    pub options: serde_json::Value,
}

// ───────────────────────────────────────────────────────────────────
// Output types (thin JSON serialisation layer)
// ───────────────────────────────────────────────────────────────────

/// A single entry in the `structure` array returned by the `parse` tool.
#[derive(Debug, Clone, Serialize)]
pub struct StructureEntry {
    pub kind: String,
    pub name: Option<String>,
    pub file: String,
    pub start: usize,
    pub end: usize,
}

// ───────────────────────────────────────────────────────────────────
// Source-name validation (REQ-STL-004)
// ───────────────────────────────────────────────────────────────────

/// Returns `true` when every byte in `name` is printable ASCII (0x20–0x7E).
fn is_printable_ascii(name: &str) -> bool {
    name.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

/// Validates the `sources` array. Returns validation-error diagnostics
/// (empty on success).
pub fn validate_sources(sources: &[SourceInput]) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let mut seen = HashSet::new();

    for src in sources {
        if src.name.is_empty() {
            errors.push(validation_diagnostic("Source name must not be empty."));
            continue;
        }
        if src.name.len() > 256 {
            errors.push(validation_diagnostic(&format!(
                "Source name '{}' exceeds 256-byte limit.",
                truncate(&src.name, 40)
            )));
        }
        if !is_printable_ascii(&src.name) {
            errors.push(validation_diagnostic(&format!(
                "Source name '{}' contains characters outside printable ASCII (0x20-0x7E).",
                src.name
                    .replace(|c: char| !c.is_ascii_graphic() && c != ' ', "?")
            )));
        }
        if !seen.insert(&src.name) {
            errors.push(validation_diagnostic(&format!(
                "Duplicate source name '{}'.",
                src.name
            )));
        }
    }
    errors
}

// ───────────────────────────────────────────────────────────────────
// Options parsing (REQ-TOL-025, REQ-TOL-026)
// ───────────────────────────────────────────────────────────────────

/// Parses and validates the `options` JSON value into `CompilerOptions`.
pub fn parse_options(value: &serde_json::Value) -> Result<CompilerOptions, Vec<Diagnostic>> {
    let obj = value
        .as_object()
        .ok_or_else(|| vec![validation_diagnostic("options must be a JSON object.")])?;

    // `dialect` is required
    let dialect_str = obj.get("dialect").and_then(|v| v.as_str()).ok_or_else(|| {
        vec![validation_diagnostic(
            "options.dialect is required and must be a string.",
        )]
    })?;

    let dialect = resolve_dialect(dialect_str).ok_or_else(|| {
        let known: Vec<String> = Dialect::ALL.iter().map(|d| d.to_string()).collect();
        vec![validation_diagnostic(&format!(
            "Unknown dialect '{}'. Known dialects: {}",
            dialect_str,
            known.join(", ")
        ))]
    })?;

    let mut options = CompilerOptions::from_dialect(dialect);

    // Build a lookup for known feature-flag keys
    let feature_keys: std::collections::HashMap<&str, usize> = CompilerOptions::FEATURE_DESCRIPTORS
        .iter()
        .enumerate()
        .map(|(i, fd)| (fd.option_key, i))
        .collect();

    let mut errors = Vec::new();

    for (key, val) in obj {
        if key == "dialect" {
            continue;
        }
        if let Some(&idx) = feature_keys.get(key.as_str()) {
            match val.as_bool() {
                Some(b) => {
                    apply_flag(&mut options, idx, b);
                }
                None => {
                    errors.push(validation_diagnostic(&format!(
                        "Option '{}' must be a boolean.",
                        key
                    )));
                }
            }
        } else {
            errors.push(validation_diagnostic(&format!(
                "Unknown option key '{}'.",
                key
            )));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(options)
}

/// Resolves a dialect string (e.g. `"iec61131-3-ed2"`) to a `Dialect`.
fn resolve_dialect(s: &str) -> Option<Dialect> {
    Dialect::ALL.iter().find(|d| d.to_string() == s).copied()
}

/// Applies a boolean feature flag override by index into `FEATURE_DESCRIPTORS`.
fn apply_flag(options: &mut CompilerOptions, idx: usize, value: bool) {
    let key = CompilerOptions::FEATURE_DESCRIPTORS[idx].option_key;
    match key {
        "allow_c_style_comments" => options.allow_c_style_comments = value,
        "allow_missing_semicolon" => options.allow_missing_semicolon = value,
        "allow_top_level_var_global" => options.allow_top_level_var_global = value,
        "allow_constant_type_params" => options.allow_constant_type_params = value,
        "allow_empty_var_blocks" => options.allow_empty_var_blocks = value,
        "allow_time_as_function_name" => options.allow_time_as_function_name = value,
        "allow_ref_to" => options.allow_ref_to = value,
        "allow_ref_arithmetic" => options.allow_ref_arithmetic = value,
        "allow_ref_stack_variables" => options.allow_ref_stack_variables = value,
        "allow_ref_type_punning" => options.allow_ref_type_punning = value,
        "allow_int_to_bool_initializer" => options.allow_int_to_bool_initializer = value,
        "allow_sizeof" => options.allow_sizeof = value,
        "allow_system_uptime_global" => options.allow_system_uptime_global = value,
        "allow_cross_family_widening" => options.allow_cross_family_widening = value,
        _ => {} // unreachable if FEATURE_DESCRIPTORS is consistent
    }
}

// ───────────────────────────────────────────────────────────────────
// Diagnostic serialisation (REQ-TOL-023)
// ───────────────────────────────────────────────────────────────────

/// Serialises a compiler `Diagnostic` to a JSON value with the fields
/// required by REQ-TOL-023: `code`, `message`, `file`, `start`, `end`,
/// `severity`.
///
/// `start` and `end` are the 0-indexed byte offsets already stored in
/// the diagnostic — no line/column conversion is performed.
pub fn serialize_diagnostic(diag: &Diagnostic) -> serde_json::Value {
    serde_json::json!({
        "code": diag.code,
        "message": diag.description(),
        "file": diag.primary.file_id.to_string(),
        "start": diag.primary.location.start,
        "end": diag.primary.location.end,
        "severity": "error",
    })
}

/// Batch-serialises compiler diagnostics to a JSON array.
pub fn serialize_diagnostics(diags: &[Diagnostic]) -> Vec<serde_json::Value> {
    diags.iter().map(serialize_diagnostic).collect()
}

// ───────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────

/// Creates a validation `Diagnostic` using the `P8001` problem code.
fn validation_diagnostic(message: &str) -> Diagnostic {
    Diagnostic::problem(
        Problem::McpInputValidation,
        Label::span(SourceSpan::default(), message),
    )
}

/// Truncates a string to at most `max` characters, appending "…" if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}…")
    }
}

// ───────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- validate_sources tests --

    #[test]
    fn validate_sources_when_empty_name_then_error() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_name_too_long_then_error() {
        let sources = vec![SourceInput {
            name: "a".repeat(257),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_name_contains_non_printable_then_error() {
        // NUL character
        let sources = vec![SourceInput {
            name: "file\0.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_name_contains_slash_then_error() {
        let sources = vec![SourceInput {
            name: "path/file.st".into(),
            content: String::new(),
        }];
        // '/' is 0x2F which IS printable ASCII, but that's fine for the
        // allowlist — the spec says printable ASCII is allowed. The old
        // denylist rejected '/' but the new allowlist permits it.
        // '/' is within 0x20-0x7E so it passes validation.
        let errs = validate_sources(&sources);
        assert!(errs.is_empty());
    }

    #[test]
    fn validate_sources_when_name_contains_tab_then_error() {
        let sources = vec![SourceInput {
            name: "file\t.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_name_contains_bell_then_error() {
        let sources = vec![SourceInput {
            name: "file\x07.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_name_contains_del_then_error() {
        // DEL is 0x7F, outside printable ASCII range
        let sources = vec![SourceInput {
            name: "file\x7F.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_duplicate_names_then_error() {
        let sources = vec![
            SourceInput {
                name: "main.st".into(),
                content: String::new(),
            },
            SourceInput {
                name: "main.st".into(),
                content: String::new(),
            },
        ];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn validate_sources_when_valid_then_ok() {
        let sources = vec![
            SourceInput {
                name: "main.st".into(),
                content: "PROGRAM p END_PROGRAM".into(),
            },
            SourceInput {
                name: "lib.st".into(),
                content: "FUNCTION f : INT END_FUNCTION".into(),
            },
        ];
        let errs = validate_sources(&sources);
        assert!(errs.is_empty());
    }

    #[test]
    fn validate_sources_when_p8001_code_then_correct() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs[0].code, "P8001");
    }

    // -- parse_options tests --

    #[test]
    fn parse_options_when_missing_dialect_then_error() {
        let val = serde_json::json!({});
        let result = parse_options(&val);
        assert!(result.is_err());
    }

    #[test]
    fn parse_options_when_unknown_dialect_then_error() {
        let val = serde_json::json!({"dialect": "cobol"});
        let result = parse_options(&val);
        assert!(result.is_err());
    }

    #[test]
    fn parse_options_when_unknown_key_then_error() {
        let val = serde_json::json!({"dialect": "iec61131-3-ed2", "bogus": true});
        let result = parse_options(&val);
        assert!(result.is_err());
    }

    #[test]
    fn parse_options_when_ed2_dialect_then_default_options() {
        let val = serde_json::json!({"dialect": "iec61131-3-ed2"});
        let opts = parse_options(&val).unwrap();
        assert!(!opts.allow_iec_61131_3_2013);
        assert!(!opts.allow_c_style_comments);
    }

    #[test]
    fn parse_options_when_ed3_dialect_then_edition3_enabled() {
        let val = serde_json::json!({"dialect": "iec61131-3-ed3"});
        let opts = parse_options(&val).unwrap();
        assert!(opts.allow_iec_61131_3_2013);
        assert!(!opts.allow_c_style_comments);
    }

    #[test]
    fn parse_options_when_rusty_dialect_then_vendor_flags_enabled() {
        let val = serde_json::json!({"dialect": "rusty"});
        let opts = parse_options(&val).unwrap();
        assert!(opts.allow_c_style_comments);
        assert!(opts.allow_missing_semicolon);
        assert!(opts.allow_ref_to);
    }

    #[test]
    fn parse_options_when_flag_override_then_applied() {
        let val = serde_json::json!({"dialect": "iec61131-3-ed2", "allow_c_style_comments": true});
        let opts = parse_options(&val).unwrap();
        assert!(opts.allow_c_style_comments);
    }

    // -- serialize_diagnostic tests --

    #[test]
    fn serialize_diagnostic_when_called_then_has_byte_offsets() {
        let diag = Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(SourceSpan::range(10, 15), "test"),
        );
        let json = serialize_diagnostic(&diag);
        assert_eq!(json["start"], 10);
        assert_eq!(json["end"], 15);
    }

    #[test]
    fn serialize_diagnostic_when_called_then_has_code_and_message() {
        let diag = Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(SourceSpan::range(0, 1), "here"),
        );
        let json = serialize_diagnostic(&diag);
        assert_eq!(json["code"], "P0002");
        assert!(json["message"].as_str().unwrap().contains("Syntax error"));
    }
}
