//! Shared helpers for semantic rules.
//!
//! Most semantic rules follow the same shape: build a [`Visitor`] that
//! accumulates [`Diagnostic`]s, walk the [`Library`], and turn the collected
//! diagnostics into a [`SemanticResult`]. This module captures that boilerplate
//! so individual rules only need to describe how to construct their visitor and
//! how to surrender the diagnostics it collected.

use ironplc_dsl::{common::Library, diagnostic::Diagnostic, visitor::Visitor};
use std::convert::Infallible;

use crate::result::SemanticResult;

/// A [`Visitor`] that accumulates diagnostics while walking a [`Library`].
///
/// Implement this for a rule's visitor to opt into [`run_rule`], which drives
/// the walk and converts the collected diagnostics into a [`SemanticResult`].
///
/// The visitor's error type is [`Infallible`] on purpose. A rule that reports a
/// problem by returning `Err` unwinds the walk, so everything the rule would
/// have found in the rest of the library is silently dropped --- and because
/// `xform_toposort_declarations` reorders declarations, which single problem
/// survives is not even the first one in the file. That was issue #1566: ten
/// rules ended with `walk(lib).map_err(|e| vec![e])`, a wrapper that reads like
/// a collection but can only ever hold one element. With no inhabited error
/// type there is no early return to write: the only way for a rule to report
/// anything is to push onto the accumulator that [`into_diagnostics`] hands
/// back, so it keeps walking and finds the rest.
///
/// A rule that genuinely cannot proceed --- as opposed to one that has found a
/// problem --- should say so with a [`Diagnostic`] and stop descending into the
/// node it could not make sense of, not stop the walk.
///
/// [`into_diagnostics`]: DiagnosticVisitor::into_diagnostics
pub(crate) trait DiagnosticVisitor: Visitor<Infallible, Value = ()> {
    /// Consumes the visitor, returning the diagnostics it accumulated.
    fn into_diagnostics(self) -> Vec<Diagnostic>;
}

/// Walks `lib` with `visitor` and converts the outcome into a [`SemanticResult`].
///
/// The walk itself cannot fail (see [`DiagnosticVisitor`]), so the visitor's
/// accumulated diagnostics alone decide the result: empty means success, any
/// diagnostics mean failure.
pub(crate) fn run_rule<V: DiagnosticVisitor>(mut visitor: V, lib: &Library) -> SemanticResult {
    match visitor.walk(lib) {
        Ok(()) => {}
        // Unreachable: `Infallible` has no values, so this arm exists only to
        // discharge the `Result`.
        Err(never) => match never {},
    }

    let diagnostics = visitor.into_diagnostics();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
