//! Shared types and helpers for MCP tools that accept `sources` and `options`.
//!
//! Every source-accepting tool (parse, check, compile, run, …) reuses
//! the types and validation functions defined here.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::{CompilerOptions, Dialect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ───────────────────────────────────────────────────────────────────
// Input types
// ───────────────────────────────────────────────────────────────────

/// A single source file supplied by the caller.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SourceInput {
    /// Logical file name (e.g. `"main.st"`). Must be non-empty, ≤ 256 bytes,
    /// and must not contain NUL, `/`, or `\`.
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
// Output types
// ───────────────────────────────────────────────────────────────────

/// A JSON-serialisable diagnostic (REQ-TOL-023).
#[derive(Debug, Clone, Serialize)]
pub struct McpDiagnostic {
    pub code: String,
    pub message: String,
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub severity: String,
}

/// A single entry in the `structure` array returned by the `parse` tool.
#[derive(Debug, Clone, Serialize)]
pub struct StructureEntry {
    pub kind: String,
    pub name: Option<String>,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

// ───────────────────────────────────────────────────────────────────
// Source-name validation (REQ-STL-004)
// ───────────────────────────────────────────────────────────────────

/// Validates the `sources` array. Returns validation-error diagnostics
/// (empty on success).
pub fn validate_sources(sources: &[SourceInput]) -> Vec<McpDiagnostic> {
    let mut errors = Vec::new();
    let mut seen = HashSet::new();

    for src in sources {
        if src.name.is_empty() {
            errors.push(validation_error("Source name must not be empty."));
            continue;
        }
        if src.name.len() > 256 {
            errors.push(validation_error(&format!(
                "Source name '{}' exceeds 256-byte limit.",
                truncate(&src.name, 40)
            )));
        }
        if src.name.contains('\0') {
            errors.push(validation_error(&format!(
                "Source name '{}' contains NUL character.",
                src.name
            )));
        }
        if src.name.contains('/') {
            errors.push(validation_error(&format!(
                "Source name '{}' contains '/'.",
                src.name
            )));
        }
        if src.name.contains('\\') {
            errors.push(validation_error(&format!(
                "Source name '{}' contains '\\'.",
                src.name
            )));
        }
        if !seen.insert(&src.name) {
            errors.push(validation_error(&format!(
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
pub fn parse_options(value: &serde_json::Value) -> Result<CompilerOptions, Vec<McpDiagnostic>> {
    let obj = value
        .as_object()
        .ok_or_else(|| vec![validation_error("options must be a JSON object.")])?;

    // `dialect` is required
    let dialect_str = obj.get("dialect").and_then(|v| v.as_str()).ok_or_else(|| {
        vec![validation_error(
            "options.dialect is required and must be a string.",
        )]
    })?;

    let dialect = resolve_dialect(dialect_str).ok_or_else(|| {
        let known: Vec<String> = Dialect::ALL.iter().map(|d| d.to_string()).collect();
        vec![validation_error(&format!(
            "Unknown dialect '{}'. Known dialects: {}",
            dialect_str,
            known.join(", ")
        ))]
    })?;

    let mut options = CompilerOptions::from_dialect(dialect);

    // Build a lookup for known feature-flag keys
    let feature_keys: HashMap<&str, usize> = CompilerOptions::FEATURE_DESCRIPTORS
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
                    errors.push(validation_error(&format!(
                        "Option '{}' must be a boolean.",
                        key
                    )));
                }
            }
        } else {
            errors.push(validation_error(&format!("Unknown option key '{}'.", key)));
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
// Diagnostic mapping (REQ-TOL-023)
// ───────────────────────────────────────────────────────────────────

/// Converts a compiler `Diagnostic` to an `McpDiagnostic`.
///
/// `source_map` maps `FileId` to the source text used for byte→line/col
/// conversion. When the file ID is not found, line 1 / col 1 is used.
pub fn map_diagnostic(diag: &Diagnostic, source_map: &HashMap<FileId, &str>) -> McpDiagnostic {
    let file_id = &diag.primary.file_id;
    let file_name = file_id.to_string();
    let source = source_map.get(file_id).copied();

    let (start_line, start_col) = byte_offset_to_line_col(source, diag.primary.location.start);
    let (end_line, end_col) = byte_offset_to_line_col(source, diag.primary.location.end);

    McpDiagnostic {
        code: diag.code.clone(),
        message: diag.description(),
        file: file_name,
        start_line,
        start_col,
        end_line,
        end_col,
        severity: "error".to_string(),
    }
}

/// Batch-converts compiler diagnostics to MCP diagnostics.
pub fn map_diagnostics(
    diags: &[Diagnostic],
    source_map: &HashMap<FileId, &str>,
) -> Vec<McpDiagnostic> {
    diags
        .iter()
        .map(|d| map_diagnostic(d, source_map))
        .collect()
}

/// Converts a 0-indexed byte offset into 1-indexed (line, column) using
/// the source text. Column counts Unicode scalar values (not bytes). A tab
/// counts as one column.
///
/// Returns `(1, 1)` when `source` is `None` (file text unavailable).
fn byte_offset_to_line_col(source: Option<&str>, offset: usize) -> (u32, u32) {
    let source = match source {
        Some(s) => s,
        None => return (1, 1),
    };

    let mut line: u32 = 1;
    let mut col: u32 = 1;

    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

// ───────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────

/// Creates a validation-level MCP diagnostic (no file / position).
fn validation_error(message: &str) -> McpDiagnostic {
    McpDiagnostic {
        code: "MCP-VALIDATION".to_string(),
        message: message.to_string(),
        file: String::new(),
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 0,
        severity: "error".to_string(),
    }
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
        assert!(errs[0].message.contains("empty"));
    }

    #[test]
    fn validate_sources_when_name_too_long_then_error() {
        let sources = vec![SourceInput {
            name: "a".repeat(257),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("256"));
    }

    #[test]
    fn validate_sources_when_name_contains_slash_then_error() {
        let sources = vec![SourceInput {
            name: "path/file.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("/"));
    }

    #[test]
    fn validate_sources_when_name_contains_backslash_then_error() {
        let sources = vec![SourceInput {
            name: "path\\file.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("\\"));
    }

    #[test]
    fn validate_sources_when_name_contains_nul_then_error() {
        let sources = vec![SourceInput {
            name: "file\0.st".into(),
            content: String::new(),
        }];
        let errs = validate_sources(&sources);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("NUL"));
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
        assert!(errs[0].message.contains("Duplicate"));
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

    // -- parse_options tests --

    #[test]
    fn parse_options_when_missing_dialect_then_error() {
        let val = serde_json::json!({});
        let result = parse_options(&val);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].message.contains("dialect"));
    }

    #[test]
    fn parse_options_when_unknown_dialect_then_error() {
        let val = serde_json::json!({"dialect": "cobol"});
        let result = parse_options(&val);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].message.contains("Unknown dialect"));
    }

    #[test]
    fn parse_options_when_unknown_key_then_error() {
        let val = serde_json::json!({"dialect": "iec61131-3-ed2", "bogus": true});
        let result = parse_options(&val);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0]
            .message
            .contains("Unknown option key"));
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

    // -- byte_offset_to_line_col / map_diagnostic tests --

    #[test]
    fn map_diagnostic_when_single_line_then_correct_line_col() {
        // "PROGRAM p"
        //  0123456789
        // offset 8 = 'p' → line 1, col 9
        let source = "PROGRAM p END_PROGRAM";
        let (line, col) = byte_offset_to_line_col(Some(source), 8);
        assert_eq!(line, 1);
        assert_eq!(col, 9);
    }

    #[test]
    fn map_diagnostic_when_multi_line_then_correct_line_col() {
        let source = "PROGRAM p\nVAR\nEND_VAR\nEND_PROGRAM";
        // Line 1: "PROGRAM p\n" (10 chars)
        // Line 2: "VAR\n"       (4 chars)
        // Line 3: "END_VAR\n"   (8 chars)
        // Line 4: "END_PROGRAM"
        // offset 14 = "END_VAR" line 3, col 1
        let (line, col) = byte_offset_to_line_col(Some(source), 14);
        assert_eq!(line, 3);
        assert_eq!(col, 1);
    }

    #[test]
    fn map_diagnostic_when_unicode_then_counts_scalar_values() {
        // "äb" — 'ä' is 2 bytes (U+00E4), 'b' is 1 byte
        // byte offset 2 = 'b' → line 1, col 2
        let source = "äb";
        let (line, col) = byte_offset_to_line_col(Some(source), 2);
        assert_eq!(line, 1);
        assert_eq!(col, 2);
    }

    #[test]
    fn map_diagnostic_when_tab_then_counts_as_one_column() {
        let source = "\tx";
        // '\t' is 1 byte, offset 1 = 'x' → line 1, col 2
        let (line, col) = byte_offset_to_line_col(Some(source), 1);
        assert_eq!(line, 1);
        assert_eq!(col, 2);
    }
}
