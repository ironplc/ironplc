//! Provides definition for diagnostics, which are normally errors and warnings
//! associated with compilation.
//!
//! There exist crates that make this easy, but we need different information
//! for different integrations and there is no one crate that does it all
//! (especially one that works for both command line and language server
//! protocol).

use std::ops::Range;

use ironplc_problems::Problem;

use crate::core::{FileId, Id, SourceLoc, SourcePosition};

/// A position marker that has both line and offset information.
#[derive(Debug)]
pub struct QualifiedPosition {
    /// Line (1-indexed)
    pub line: usize,

    /// Column (1-indexed)
    pub column: usize,

    /// Byte offset from start of string (0-indexed)
    pub offset: usize,
}

impl QualifiedPosition {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

/// A position marker that only has an offset in a file.
#[derive(Debug)]
pub struct OffsetRange {
    /// Byte offset from start of string (0-indexed)
    pub start: usize,
    /// Byte offset from end of string (0-indexed)
    pub end: usize,
}

#[derive(Debug)]
pub enum Location {
    QualifiedPosition(QualifiedPosition),
    OffsetRange(OffsetRange),
}

/// A label that refers to some range in a file and possibly associated
/// with a message related to that range.
///
/// Normally this indicates the location of an error or warning along with a
/// text message describing that position.
#[derive(Debug)]
pub struct Label {
    /// The position of label.
    pub location: Location,

    /// Identifier for the file.
    pub file_id: FileId,

    /// A message describing this label.
    pub message: String,
}

impl Label {
    pub fn qualified(
        file_id: impl Into<FileId>,
        position: QualifiedPosition,
        message: impl Into<String>,
    ) -> Self {
        Self {
            location: Location::QualifiedPosition(position),
            file_id: file_id.into(),
            message: message.into(),
        }
    }

    pub fn offset(
        file_id: impl Into<FileId>,
        offset: impl Into<Range<usize>>,
        message: impl Into<String>,
    ) -> Self {
        let range = offset.into();
        Self {
            location: Location::OffsetRange(OffsetRange {
                start: range.start,
                end: range.end,
            }),
            file_id: file_id.into(),
            message: message.into(),
        }
    }

    pub fn source_loc(source_loc: &SourceLoc, message: impl Into<String>) -> Self {
        Self {
            location: Location::OffsetRange(OffsetRange {
                start: source_loc.start,
                end: source_loc.end,
            }),
            file_id: source_loc.file_id.clone(),
            message: message.into(),
        }
    }

    /// A "position" that a file in it's entirety rather that a particular
    /// line number.
    pub fn file(file_id: impl Into<FileId>, message: impl Into<String>) -> Self {
        Self {
            location: Location::QualifiedPosition(QualifiedPosition {
                column: 0,
                line: 0,
                offset: 0,
            }),
            file_id: file_id.into(),
            message: message.into(),
        }
    }
}

/// A diagnostic. Diagnostic have a code that is indicative of the category,
/// a primary location and possibly non-zero set of secondary location.
#[derive(Debug)]
pub struct Diagnostic {
    /// A normally unique value describing the type of diagnostic.
    pub code: String,

    description: String,

    /// The primary or first diagnostic.
    pub primary: Label,

    /// Additional descriptions to the constant description.
    pub described: Vec<String>,

    /// Additional information about the diagnostic.
    pub secondary: Vec<Label>,
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
            secondary: vec![],
        }
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code.
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    pub fn todo(file: &str, line: u32) -> Self {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::source_loc(
                &SourceLoc::default(),
                format!("Not implemented at {}#L{}", file, line),
            ),
        )
    }

    /// Creates a "todo" diagnostic associated with a file and line in the Rust
    /// source code. Also provides a location in IEC 61131-3 associated with the
    /// todo (but is not necessarily the origin).
    ///
    /// Unlike other uses of problem, the location in this is related to the compiler
    /// rather than the IEC 61131-3 source.
    pub fn todo_with_id(id: &Id, file: &str, line: u32) -> Self {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::source_loc(
                id.position(),
                format!("Not implemented at {}#L{}", file, line),
            ),
        )
    }

    /// Adds to the problem description (primary text) additional context
    /// about the problem.
    ///
    /// This is similar to adding primary and second items except that this
    /// forms part of the main description and does not need to be related to
    /// a position in a source file.
    pub fn with_context(mut self, description: &str, item: &String) -> Self {
        self.described.push(format!("{}={}", description, item));
        self
    }

    /// Adds to the problem description (primary text) additional context
    /// about the problem.
    ///
    /// This is similar to adding primary and second items except that this
    /// forms part of the main description and does not need to be related to
    /// a position in a source file.
    pub fn with_context_id(mut self, description: &str, item: &Id) -> Self {
        self.described.push(format!("{}={}", description, item));
        self
    }

    pub fn with_secondary(mut self, label: Label) -> Self {
        self.secondary.push(label);
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
}

impl From<Diagnostic> for () {
    fn from(value: Diagnostic) -> Self {
        // Just drop the diagnostic!
    }
}
