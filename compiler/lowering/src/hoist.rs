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
//! Running the parts first keeps the order the source runs things in only
//! when nothing else in the value runs before a call. So an assignment with
//! parts is lowered only when it is an `Assign` of a scalar value, and its
//! parts and value read the results as the values of a stack
//! (`reads_results_in_order`, REQ-LOW-lowering-174). Any other is not
//! lowered: `r := a + F(x := 1, n => a)` reads `a` before the call that
//! writes it, and `x.3 := F(x := 1, n => x) > 0` reads `x` before the call on
//! the route that reads the AST.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::Assignment;
use ironplc_ir::lowered::{reads_results_in_order, Place, Stmt, StmtKind};

use crate::variables::BodyFrame;
use crate::Lowering;

/// The parts lowering has made for the calls with outputs of the assignment
/// being lowered.
pub(crate) struct Hoisting {
    /// The assignment's span, which every part carries, for the line map
    /// (REQ-LOW-lowering-063).
    span: SourceSpan,
    /// The address of each call, the temporary that holds its result and its
    /// part, in the order lowering made them.
    parts: Vec<(usize, Place, Vec<Stmt>)>,
}

impl Hoisting {
    /// The span every part carries.
    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// The temporary that holds the result of the call at `address`, if
    /// lowering has made its part.
    pub(crate) fn result(&self, address: usize) -> Option<&Place> {
        self.parts
            .iter()
            .find_map(|(hoisted, result, _)| (*hoisted == address).then_some(result))
    }

    /// Adds the part of the call at `address`, whose result `result` holds.
    pub(crate) fn push(&mut self, address: usize, part: Vec<Stmt>, result: Place) {
        self.parts.push((address, result, part));
    }

    /// Whether running the parts, then `stored`, keeps the order the source
    /// runs them in (REQ-LOW-lowering-174): `stored` assigns a scalar value,
    /// and the parts and that value read the results as the values of a
    /// stack. An `AssignBits` reads its place before its value on the route
    /// that reads the AST.
    fn keeps_order(&self, stored: &Stmt) -> bool {
        let StmtKind::Assign { value, .. } = stored.kind() else {
            return false;
        };
        let mut parts = Vec::with_capacity(self.parts.len());
        for (_, result, part) in &self.parts {
            let Some(StmtKind::Assign { value, .. }) = part.first().map(Stmt::kind) else {
                return false;
            };
            parts.push((value, result.root()));
        }
        reads_results_in_order(&parts, value)
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
        let enclosing = self.hoisting.replace(Hoisting {
            span: assignment.span(),
            parts: Vec::new(),
        });
        let stored = self.assignment(frame, assignment);
        let hoisting = std::mem::replace(&mut self.hoisting, enclosing);
        let (Some(stored), Some(hoisting)) = (stored?, hoisting) else {
            return Ok(None);
        };
        if hoisting.parts.is_empty() {
            return Ok(Some(vec![stored]));
        }
        if !hoisting.keeps_order(&stored) {
            return Ok(None);
        }
        let mut stmts: Vec<Stmt> = hoisting
            .parts
            .into_iter()
            .flat_map(|(_, _, part)| part)
            .collect();
        stmts.push(stored);
        Ok(Some(stmts))
    }
}
