use core::fmt;

use codespan_reporting::diagnostic::{Diagnostic, Label};
use ironplc_dsl::core::SourceLoc;

#[derive(Debug)]
pub struct SemanticDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub label: Vec<(String, SourceLoc)>,
}

impl SemanticDiagnostic {
    pub fn error(code: &'static str, message: String) -> SemanticDiagnostic {
        SemanticDiagnostic {
            code,
            message,
            label: vec![],
        }
    }

    pub fn with_location(mut self, loc: &SourceLoc) -> SemanticDiagnostic {
        self.label.push(("".to_string(), loc.clone()));
        self
    }

    /// Adds a label to the error indicating a location and description of the position.
    pub fn with_label(mut self, loc: &Option<SourceLoc>, message: &str) -> SemanticDiagnostic {
        match loc {
            Some(loc) => self.label.push((message.to_string(), loc.clone())),
            None => {}
        }
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
        let labels = si
            .label
            .iter()
            .map(|label| {
                let start = label.1.start;
                let msg = &label.0;
                match label.1.end {
                    Some(end) => Label::primary((), start..end).with_message(msg.as_str()),
                    None => Label::primary((), start..start).with_message(msg.as_str()),
                }
            })
            .collect();

        Diagnostic::error()
            .with_message(si.message)
            .with_code(si.code)
            .with_labels(labels)
    }
}
