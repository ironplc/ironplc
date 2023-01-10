use std::collections::HashSet;

use codespan_reporting::diagnostic::{Diagnostic, Label};

#[derive(Debug, PartialEq)]
pub struct Location {
    /// Line (1-indexed)
    pub line: usize,

    /// Column (1-indexed)
    pub column: usize,

    /// Byte offset from start of string (0-indexed)
    pub offset: usize,
}

#[derive(Debug, PartialEq)]
pub struct ParserDiagnostic {
    pub location: Location,
    pub expected: HashSet<&'static str>,
}

impl From<ParserDiagnostic> for Diagnostic<()> {
    fn from(pi: ParserDiagnostic) -> Self {
        let start = pi.location.offset;
        Diagnostic::error()
            .with_message("Error parsing library")
            .with_code("E0001")
            .with_labels(vec![
                Label::primary((), start..start).with_message("Most likely location")
            ])
            .with_notes(vec![format!("Expected one of: {:?}", pi.expected)])
    }
}
