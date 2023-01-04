use core::fmt;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use ironplc_dsl::core::SourceLoc;

#[derive(Debug)]
pub struct SemanticDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub location: Option<SourceLoc>,
}

impl SemanticDiagnostic {
    pub fn error(code: &'static str, message: String) -> SemanticDiagnostic {
        SemanticDiagnostic {
            code: code,
            message: message,
            location: None,
        }
    }

    pub fn with_location(mut self, loc: &SourceLoc) -> SemanticDiagnostic {
        self.location = Some(loc.clone());
        self
    }
}

impl fmt::Display for SemanticDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SemanticDiagnostic")
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

impl From<SemanticDiagnostic> for Diagnostic<()> {
    fn from(si: SemanticDiagnostic) -> Self {
        let mut diagnostic = Diagnostic::error()
            .with_message(si.message)
            .with_code(si.code);

        if let Some(loc) = si.location {
            let start = loc.offset;
            diagnostic = diagnostic.with_labels(vec![
                Label::primary((), start..start).with_message("sematic error")
            ])
        }

        diagnostic
    }
}
