//! The diagnostic a constructor returns when its operands break an invariant.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

/// A P9998 naming the `constructor` that refused to build a node, and what
/// was wrong with its operands (REQ-LOW-ir-081).
#[track_caller]
pub(crate) fn broken(constructor: &str, span: &SourceSpan, what: impl AsRef<str>) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        format!("{constructor}: {}", what.as_ref()),
    ))
}
