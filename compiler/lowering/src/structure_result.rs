//! Calls of functions whose result is a structure.
//!
//! A function whose result is a structure writes it through its result
//! parameter, a reference to a temporary its caller owns, as one whose result
//! is a string does (REQ-LOW-lowering-079). Such a call is assigned whole to
//! a structure, `p := MK(v := 3, n => k)`, and lowers to three steps, in
//! order: a `Call` statement that passes a new temporary for the result, the
//! stores of its outputs, if any, then a `Copy` of the temporary to the
//! target (REQ-LOW-lowering-186).
//!
//! Not lowered yet: such a call anywhere else, such as an argument of
//! another call, and a method whose result is a structure.

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Assignment, ExprKind as AstExprKind};
use ironplc_ir::lowered::{Stmt, TypeKind};

use crate::function::UserCall;
use crate::variables::BodyFrame;
use crate::{Body, Lowering};

impl Lowering<'_> {
    /// The statements `assignment` lowers to when it assigns a call of a
    /// function whose result is a structure to a structure: the call, the
    /// stores of its outputs, then the copy of its result. `None` for any
    /// other assignment, and for one lowering does not support yet.
    pub(crate) fn structure_result_assignment(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
    ) -> Result<Option<Vec<Stmt>>, Diagnostic> {
        let AstExprKind::Function(func) = &assignment.value.kind else {
            return Ok(None);
        };
        if assignment.set_bind || assignment.reset_bind || assignment.deref {
            return Ok(None);
        }
        let Some(Body::Function(function)) = self.body_named(&func.name) else {
            return Ok(None);
        };
        let Some(target) = self.variable_place(frame, &assignment.target)? else {
            return Ok(None);
        };
        if !matches!(
            self.program.types().kind(target.ty()),
            Some(TypeKind::Structure { .. })
        ) {
            return Ok(None);
        }
        let span = assignment.span();
        let call = UserCall::Function {
            call: func,
            function,
        };
        let Some((args, stores)) = self.call_with_stores(frame, call, &span)? else {
            return Ok(None);
        };
        let Some(result) = args.result.clone() else {
            return Ok(None);
        };
        let mut statements = vec![self.program.call_stmt(args.pou, args.args, span.clone())?];
        statements.extend(stores.unwrap_or_default());
        statements.push(self.program.copy(target, result, span)?);
        Ok(Some(statements))
    }
}
