//! Calls of functions and methods with outputs, lowered ahead of the
//! assignment whose value they are in (REQ-LOW-lowering-172).
//!
//! The value of a call with outputs has to wait while its outputs are
//! stored, and a temporary is where it waits. So while an assignment is
//! lowered, a call with outputs that lowering meets in it is lowered where
//! it is met, as its *part*: the call stored in a temporary of its result's
//! type, then the assignment of each output from the temporary passed for it
//! (`function.rs`). The call's value is a read of the result's temporary. The
//! assignment lowers to the parts, in the order lowering made them, then
//! itself, each with the assignment's span.
//!
//! Only a call that is the whole value of the assignment, inside the
//! conversions the analyzer recorded, is lowered so yet: an assignment that
//! makes any other part is not lowered.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::Assignment;
use ironplc_ir::lowered::{Place, Stmt};

use crate::variables::BodyFrame;
use crate::Lowering;

/// The parts lowering has made for the calls with outputs of the assignment
/// being lowered.
pub(crate) struct Hoisting {
    /// The assignment's span, which every part carries, for the line map
    /// (REQ-LOW-lowering-063).
    span: SourceSpan,
    /// The parts, in the order lowering made them.
    parts: Vec<Stmt>,
    /// The temporary that holds each call's result, by the call's address.
    results: Vec<(usize, Place)>,
}

impl Hoisting {
    /// The span every part carries.
    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// The temporary that holds the result of the call at `address`, if
    /// lowering has made its part.
    pub(crate) fn result(&self, address: usize) -> Option<&Place> {
        self.results
            .iter()
            .find_map(|(hoisted, result)| (*hoisted == address).then_some(result))
    }

    /// Adds the part of the call at `address`, whose result `result` holds.
    pub(crate) fn push(&mut self, address: usize, part: Vec<Stmt>, result: Place) {
        self.parts.extend(part);
        self.results.push((address, result));
    }
}

impl Lowering<'_> {
    /// The assignment statement `assignment` in the body of `frame`: the
    /// parts of the calls with outputs in its value, then the assignment
    /// (REQ-LOW-lowering-092, REQ-LOW-lowering-172).
    pub(crate) fn assignment_statement(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
    ) -> Result<Option<Vec<Stmt>>, Diagnostic> {
        let Some(call) = self.call_with_outputs(frame, &assignment.value) else {
            return Ok(self.assignment(frame, assignment)?.map(|stmt| vec![stmt]));
        };
        let address = call.address();
        let enclosing = self.hoisting.replace(Hoisting {
            span: assignment.span(),
            parts: Vec::new(),
            results: Vec::new(),
        });
        let stored = self.assignment(frame, assignment);
        let hoisting = std::mem::replace(&mut self.hoisting, enclosing);
        let (Some(stored), Some(hoisting)) = (stored?, hoisting) else {
            return Ok(None);
        };
        // Only the call that is the whole value has a part yet.
        if !matches!(hoisting.results.as_slice(), [(hoisted, _)] if *hoisted == address) {
            return Ok(None);
        }
        let mut stmts = hoisting.parts;
        stmts.push(stored);
        Ok(Some(stmts))
    }
}
