use core::fmt;

use codespan_reporting::diagnostic::Diagnostic;

#[derive(Debug)]
pub struct SemanticDiagnostic {
    pub code: &'static str,
    pub message: String,
}

impl SemanticDiagnostic {
    pub fn error(code: &'static str, message: String) -> Result<(), SemanticDiagnostic> {
        Err(SemanticDiagnostic {
            code: code,
            message: message,
        })
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
        Diagnostic::error()
            .with_message(si.message)
            .with_code(si.code)
    }
}
