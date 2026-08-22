//! Provides definition for diagnostics, which are normally errors and warnings
//! associated with compilation.
//!
//! # Diagnostic Creation Guidelines
//!
//! When creating diagnostics:
//! 1. Use `Problem::SomeCode` from the shared problem system
//! 2. Primary label points to the main error location
//! 3. Secondary labels point to different, related locations
//! 4. Labels describe what's at each location, not how to fix issues
//!
//! There exist crates that make this easy, but we need different information
//! for different integrations and there is no one crate that does it all
//! (especially one that works for both command line and language server
//! protocol).
use ironplc_problems::Problem;
use std::collections::HashSet;

use crate::common::TypeName;
use crate::core::{FileId, Id, Located, SourceSpan};

/// A position marker that only has an offset in a file.
#[derive(Debug, Clone)]
pub struct Location {
    /// Byte offset from start of string (0-indexed)
    pub start: usize,
    /// Byte offset from end of string (0-indexed)
    pub end: usize,
}

/// A 0-indexed line and column within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    /// 0-indexed line number.
    pub line: u32,
    /// 0-indexed column number (code points, not bytes).
    pub column: u32,
}

impl LineColumn {
    /// Converts a byte offset in `source` to a 0-indexed line and column.
    ///
    /// The column is measured in code points. Offsets beyond the end of
    /// `source` are clamped to the end of `source`.
    pub fn from_offset(source: &str, offset: usize) -> Self {
        let clamped = offset.min(source.len());
        let mut line: u32 = 0;
        let mut column: u32 = 0;
        for ch in source[..clamped].chars() {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        LineColumn { line, column }
    }
}

/// A label that refers to some range in a file and possibly associated
/// with a message related to that range.
///
/// Normally this indicates the location of an error or warning along with a
/// text message describing that position.
#[derive(Debug, Clone)]
pub struct Label {
    /// The position of label.
    pub location: Location,

    /// Identifier for the file.
    pub file_id: FileId,

    /// A message describing this label.
    pub message: String,
}

impl Label {
    /// Creates a label pointing to a source span with a descriptive message.
    ///
    /// The message should describe **what is at this location**, not provide
    /// explanations or fix guidance.
    ///
    /// # Examples
    /// ```text
    /// Label::span(name.span(), "Type declaration")     // ✅ What's there
    /// Label::span(base.span(), "Base type")           // ✅ What's there
    /// Label::span(name.span(), "Fix by adding type")  // ❌ Fix guidance
    /// ```
    pub fn span(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            location: Location {
                start: span.start,
                end: span.end,
            },
            file_id: span.file_id,
            message: message.into(),
        }
    }

    /// A "position" that a file in it's entirety rather that a particular
    /// line number.
    pub fn file(file_id: impl Into<FileId>, message: impl Into<String>) -> Self {
        Self {
            location: Location { start: 0, end: 0 },
            file_id: file_id.into(),
            message: message.into(),
        }
    }
}

/// A diagnostic. Diagnostic have a code that is indicative of the category,
/// a primary location and possibly non-zero set of secondary location.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// A normally unique value describing the type of diagnostic.
    pub code: String,

    description: String,

    /// The primary or first diagnostic.
    pub primary: Label,

    /// Additional descriptions to the constant description.
    pub described: Vec<String>,

    /// Guidance on how to resolve the problem. Unlike labels (which describe
    /// *what* is at a location), help notes explain *how to fix* the problem.
    /// Rendered as trailing notes by the CLI and appended to the message by
    /// the language server.
    pub help: Vec<String>,

    /// Additional information about the diagnostic.
    pub secondary: Vec<Label>,

    /// Rust source file that produced this diagnostic (from `file!()` macro).
    pub source_file: Option<String>,

    /// Rust source line that produced this diagnostic (from `line!()` macro).
    pub source_line: Option<u32>,
}

/// Formats the label message for a "not implemented" diagnostic from the
/// compiler location that produced it.
fn not_implemented_message(caller: &std::panic::Location<'_>) -> String {
    format!("Not implemented at {}#L{}", caller.file(), caller.line())
}

impl Diagnostic {
    /// Creates a diagnostic from the problem code and with the specified label.
    ///
    /// The label associates the problem to a particular instance in IEC 61131-3 source
    /// file.
    pub fn problem(problem: Problem, primary: Label) -> Self {
        Self {
            code: problem.code().to_string(),
            description: problem.message().to_string(),
            primary,
            described: vec![],
            help: vec![],
            secondary: vec![],
            source_file: None,
            source_line: None,
        }
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code.
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn todo() -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(SourceSpan::default(), not_implemented_message(caller)),
        )
        .with_source(caller.file(), caller.line())
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code. Also provides a location in IEC 61131-3 associated with the
    /// todo (but is not necessarily the origin).
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn todo_with_id(id: &Id) -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(id.span(), not_implemented_message(caller)),
        )
        .with_source(caller.file(), caller.line())
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code. Also provides a location in IEC 61131-3 associated with the
    /// todo (but is not necessarily the origin).
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn todo_with_type(ty: &TypeName) -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(ty.span(), not_implemented_message(caller)),
        )
        .with_source(caller.file(), caller.line())
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code. Also provides a location in IEC 61131-3 associated with the
    /// todo (but is not necessarily the origin).
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn todo_with_span(span: SourceSpan) -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span, not_implemented_message(caller)),
        )
        .with_source(caller.file(), caller.line())
    }

    /// Creates a P9999 (NotImplemented) diagnostic that automatically records
    /// the compiler `file#Lline` of the call site as its source location.
    ///
    /// Prefer this over `Diagnostic::problem(Problem::NotImplemented, …)` (which
    /// no longer compiles): the compiler location is what the telemetry
    /// dashboards rank to point maintainers at the unimplemented code. Unlike
    /// `todo*()`, the caller supplies the primary label, so a descriptive
    /// message and IEC 61131-3 span are preserved.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn not_implemented(primary: Label) -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(Problem::NotImplemented, primary)
            .with_source(caller.file(), caller.line())
    }

    /// Creates an "internal error" diagnostic associated with a file and line in the Rust
    /// source code.
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn internal_error() -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(
            Problem::InternalError,
            Label::span(
                SourceSpan::default(),
                format!(
                    "Internal error at {}#L{} indicates a bug in the compiler",
                    caller.file(),
                    caller.line()
                ),
            ),
        )
        .with_source(caller.file(), caller.line())
    }

    /// Creates a P9998 (InternalError) diagnostic that automatically records
    /// the compiler `file#Lline` of the call site as its source location, while
    /// letting the caller supply a descriptive primary label.
    ///
    /// Prefer this over `Diagnostic::problem(Problem::InternalError, …)` (which
    /// no longer compiles). Use `internal_error()` instead when no custom label
    /// is needed.
    ///
    /// The location is captured via `#[track_caller]`, so no `file!()`/`line!()`
    /// need to be passed.
    #[track_caller]
    #[allow(deprecated)]
    pub fn internal_error_at(primary: Label) -> Self {
        let caller = std::panic::Location::caller();
        Diagnostic::problem(Problem::InternalError, primary)
            .with_source(caller.file(), caller.line())
    }

    /// Adds to the problem description (primary text) additional context
    /// about the problem.
    ///
    /// This is similar to adding primary and second items except that this
    /// forms part of the main description and does not need to be related to
    /// a position in a source file.
    pub fn with_context(mut self, description: &str, item: &String) -> Self {
        self.described.push(format!("{description}={item}"));
        self
    }

    pub fn with_context_id(mut self, description: &str, item: &Id) -> Self {
        self.described.push(format!("{description}={item}"));
        self
    }

    pub fn with_context_type(mut self, description: &str, item: &TypeName) -> Self {
        self.described.push(format!("{description}={item}"));
        self
    }

    /// Adds a secondary label pointing to a related location.
    ///
    /// Secondary labels must point to **different spans** than the primary label
    /// and should describe what is located at those spans.
    ///
    /// # Examples
    /// ```text
    /// // ✅ Correct: Different spans, describes locations
    /// Diagnostic::problem(Problem::ParentTypeNotDeclared,
    ///     Label::span(decl_span, "Type declaration"))
    /// .with_secondary(Label::span(base_span, "Base type"))
    ///
    /// // ❌ Wrong: Same span or fix guidance
    /// .with_secondary(Label::span(decl_span, "Add missing type"))
    /// ```
    pub fn with_secondary(mut self, label: Label) -> Self {
        self.secondary.push(label);
        self
    }

    /// Adds a help note describing how to resolve the problem.
    ///
    /// Help notes are for fix guidance (e.g. "use `(* *)` comments"), which is
    /// intentionally kept out of labels. They are rendered as trailing notes by
    /// the command line and appended to the message by the language server.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }

    /// Returns the help notes describing how to resolve the problem.
    pub fn help(&self) -> &[String] {
        &self.help
    }

    /// Sets the Rust source file and line that produced this diagnostic.
    ///
    /// This is typically called with `file!()` and `line!()` to record
    /// where in the compiler the diagnostic was generated.
    pub fn with_source(mut self, file: &str, line: u32) -> Self {
        self.source_file = Some(file.to_string());
        self.source_line = Some(line);
        self
    }

    /// Returns the description for the diagnostic. This may add in other
    /// data in addition that is part of the diagnostic.
    pub fn description(&self) -> String {
        if self.described.is_empty() {
            self.description.clone()
        } else {
            format!("{} ({})", self.description, self.described.join(", "))
        }
    }

    pub fn file_ids(&self) -> HashSet<&FileId> {
        let mut file_ids = HashSet::new();
        file_ids.insert(&self.primary.file_id);

        for secondary_item in self.secondary.iter() {
            file_ids.insert(&secondary_item.file_id);
        }

        file_ids
    }
}

/// The www.ironplc.com reference section that documents a problem code, derived
/// from its leading letter: `P####` → `compiler`, `V####` → `runtime`,
/// `E####` → `editor`.
///
/// Returns `"unknown"` for any unrecognized prefix rather than guessing a
/// section: a `…/reference/unknown/problems/…` URL 404s honestly instead of
/// confidently pointing at the wrong docs. The `docs_section_covers_every_documented_code`
/// test in this module walks the docs tree and fails if any documented code's
/// prefix is left unmapped, so adding a new code family without updating this
/// function is caught at test time rather than shipping a broken link.
pub fn docs_section(code: &str) -> &'static str {
    match code.chars().next() {
        Some('P') => "compiler",
        Some('V') => "runtime",
        Some('E') => "editor",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_when_called_then_creates_not_implemented_diagnostic() {
        let line = line!() + 1;
        let diag = Diagnostic::todo();
        assert_eq!(diag.code, "P9999");
        assert!(diag.primary.message.contains(file!()));
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert_eq!(diag.source_line, Some(line));
    }

    #[test]
    fn todo_with_id_when_called_then_includes_id_location() {
        let id = Id::from("my_var");
        let line = line!() + 1;
        let diag = Diagnostic::todo_with_id(&id);
        assert_eq!(diag.code, "P9999");
        assert!(diag.primary.message.contains(file!()));
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert_eq!(diag.source_line, Some(line));
    }

    #[test]
    fn todo_with_type_when_called_then_includes_type_location() {
        let ty = TypeName::from("MY_TYPE");
        let line = line!() + 1;
        let diag = Diagnostic::todo_with_type(&ty);
        assert_eq!(diag.code, "P9999");
        assert!(diag.primary.message.contains(file!()));
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert_eq!(diag.source_line, Some(line));
    }

    #[test]
    fn todo_with_span_when_called_then_includes_span() {
        let span = SourceSpan::default();
        let line = line!() + 1;
        let diag = Diagnostic::todo_with_span(span);
        assert_eq!(diag.code, "P9999");
        assert!(diag.primary.message.contains(file!()));
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert_eq!(diag.source_line, Some(line));
    }

    #[test]
    fn internal_error_when_called_then_creates_diagnostic() {
        let line = line!() + 1;
        let diag = Diagnostic::internal_error();
        assert_eq!(diag.code, "P9998");
        assert!(diag.primary.message.contains(file!()));
        assert!(diag.primary.message.contains("bug in the compiler"));
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert_eq!(diag.source_line, Some(line));
    }

    #[test]
    fn not_implemented_when_called_then_records_caller_location_and_label() {
        let diag = Diagnostic::not_implemented(Label::span(
            SourceSpan::default(),
            "custom unimplemented message",
        ));
        assert_eq!(diag.code, "P9999");
        // The caller's compiler location is captured automatically.
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert!(diag.source_line.is_some());
        // The caller-supplied label is preserved (unlike todo*()).
        assert_eq!(diag.primary.message, "custom unimplemented message");
    }

    #[test]
    fn internal_error_at_when_called_then_records_caller_location_and_label() {
        let diag = Diagnostic::internal_error_at(Label::span(
            SourceSpan::default(),
            "custom internal error message",
        ));
        assert_eq!(diag.code, "P9998");
        assert_eq!(diag.source_file.as_deref(), Some(file!()));
        assert!(diag.source_line.is_some());
        assert_eq!(diag.primary.message, "custom internal error message");
    }

    #[test]
    fn offset_to_line_column_when_at_start_then_returns_zero_zero() {
        let lc = LineColumn::from_offset("abc\ndef", 0);
        assert_eq!(lc, LineColumn { line: 0, column: 0 });
    }

    #[test]
    fn offset_to_line_column_when_on_first_line_then_returns_line_zero() {
        let lc = LineColumn::from_offset("abc\ndef", 2);
        assert_eq!(lc, LineColumn { line: 0, column: 2 });
    }

    #[test]
    fn offset_to_line_column_when_after_newline_then_advances_line() {
        let lc = LineColumn::from_offset("abc\ndef", 4);
        assert_eq!(lc, LineColumn { line: 1, column: 0 });
    }

    #[test]
    fn offset_to_line_column_when_on_later_line_then_returns_correct_column() {
        let lc = LineColumn::from_offset("abc\ndef\nghi", 9);
        assert_eq!(lc, LineColumn { line: 2, column: 1 });
    }

    #[test]
    fn offset_to_line_column_when_offset_past_end_then_clamps_to_end() {
        let lc = LineColumn::from_offset("abc", 999);
        assert_eq!(lc, LineColumn { line: 0, column: 3 });
    }

    #[test]
    fn offset_to_line_column_when_empty_source_then_returns_zero_zero() {
        let lc = LineColumn::from_offset("", 0);
        assert_eq!(lc, LineColumn { line: 0, column: 0 });
    }

    #[test]
    fn file_ids_when_has_secondary_then_returns_all_file_ids() {
        let primary_file = FileId::from_string("file1");
        let secondary_file = FileId::from_string("file2");
        let diag = Diagnostic::problem(
            Problem::SyntaxError,
            Label::file(primary_file.clone(), "primary"),
        )
        .with_secondary(Label::file(secondary_file.clone(), "secondary"));

        let ids = diag.file_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&primary_file));
        assert!(ids.contains(&secondary_file));
    }

    #[test]
    fn docs_section_when_known_prefix_then_matches_reference_section() {
        assert_eq!(docs_section("P0001"), "compiler");
        assert_eq!(docs_section("V6008"), "runtime");
        assert_eq!(docs_section("E0001"), "editor");
    }

    #[test]
    fn docs_section_when_unknown_prefix_then_unknown() {
        // A future code family must not be silently attributed to an existing
        // section; it falls back to "unknown" (a 404) until mapped.
        assert_eq!(docs_section("D0001"), "unknown");
        assert_eq!(docs_section(""), "unknown");
    }

    // Guards against a new documented code family (a new
    // docs/reference/<section>/problems/ directory) slipping past `docs_section`
    // and shipping a wrong or 404 problem-code link. Walks the real docs tree
    // and asserts every documented code's prefix maps to the section directory
    // that actually contains its page. If this fails, add the new prefix to
    // `docs_section`.
    #[test]
    fn docs_section_covers_every_documented_code() {
        use std::path::Path;

        let reference = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference");
        let mut checked = 0;

        for section_entry in std::fs::read_dir(&reference)
            .unwrap_or_else(|e| panic!("read {}: {e}", reference.display()))
            .flatten()
        {
            let section = section_entry.file_name().to_string_lossy().to_string();
            let problems_dir = section_entry.path().join("problems");
            let Ok(files) = std::fs::read_dir(&problems_dir) else {
                continue; // Not every reference section has a problems/ dir.
            };

            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().to_string();
                let Some(code) = name.strip_suffix(".rst") else {
                    continue;
                };
                // Problem-code files are <LETTER><digits> (e.g. P0001); skip
                // index.rst and any other prose pages.
                let mut chars = code.chars();
                let is_code = matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
                    && !code[1..].is_empty()
                    && code[1..].chars().all(|c| c.is_ascii_digit());
                if !is_code {
                    continue;
                }

                assert_eq!(
                    docs_section(code),
                    section,
                    "docs_section({code}) should be {section:?} (its page lives in \
                     docs/reference/{section}/problems/); a new code family needs a \
                     matching arm in docs_section",
                );
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "found no documented problem codes under {} — test wiring is broken",
            reference.display()
        );
    }
}
