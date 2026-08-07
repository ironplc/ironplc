//! Shared helpers for semantic rules.
//!
//! Most semantic rules follow the same shape: build a [`Visitor`] that
//! accumulates [`Diagnostic`]s, walk the [`Library`], and turn the collected
//! diagnostics into a [`SemanticResult`]. This module captures that boilerplate
//! so individual rules only need to describe how to construct their visitor and
//! how to surrender the diagnostics it collected.

use ironplc_dsl::{common::Library, diagnostic::Diagnostic, visitor::Visitor};

use crate::result::SemanticResult;

/// A [`Visitor`] that accumulates diagnostics while walking a [`Library`].
///
/// Implement this for a rule's visitor to opt into [`run_rule`], which drives
/// the walk and converts the collected diagnostics into a [`SemanticResult`].
pub(crate) trait DiagnosticVisitor: Visitor<Diagnostic, Value = ()> {
    /// Consumes the visitor, returning the diagnostics it accumulated.
    fn into_diagnostics(self) -> Vec<Diagnostic>;
}

/// Walks `lib` with `visitor` and converts the outcome into a [`SemanticResult`].
///
/// A hard error from the walk is returned immediately. Otherwise the visitor's
/// accumulated diagnostics decide the result: empty means success, any
/// diagnostics mean failure.
pub(crate) fn run_rule<V: DiagnosticVisitor>(mut visitor: V, lib: &Library) -> SemanticResult {
    visitor.walk(lib).map_err(|e| vec![e])?;
    let diagnostics = visitor.into_diagnostics();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
