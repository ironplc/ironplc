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

use ironplc_dsl::common::{FunctionDeclaration, FunctionReturnType};
use ironplc_dsl::core::Located;
use ironplc_dsl::textual::{Assignment, ExprKind as AstExprKind, Function};
use ironplc_ir::lowered::Stmt;

use crate::function::UserCall;
use crate::variables::BodyFrame;
use crate::{Body, Lowered, Lowering, Unlowered};

impl<'a> Lowering<'a> {
    /// The call `assignment` assigns whole, with the function it calls,
    /// when the function is user-defined and its result is a structure;
    /// `None` for any other assignment. One through `S=`, `R=` or `^` is
    /// not one lowering expresses.
    pub(crate) fn structure_result_call<'v>(
        &self,
        assignment: &'v Assignment,
    ) -> Option<(&'v Function, &'a FunctionDeclaration)> {
        let AstExprKind::Function(func) = &assignment.value.kind else {
            return None;
        };
        if assignment.set_bind || assignment.reset_bind || assignment.deref {
            return None;
        }
        let Some(Body::Function(function)) = self.body_named(&func.name) else {
            return None;
        };
        let FunctionReturnType::Named(result) = &function.return_type else {
            return None;
        };
        self.analysis
            .context()
            .types()
            .resolve_struct_type(result)?;
        Some((func, function))
    }

    /// The statements `assignment`, which assigns the call `func` of
    /// `function`, a function whose result is a structure, lowers to: the
    /// call, the stores of its outputs, then the copy of its result.
    pub(crate) fn structure_result_assignment(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Lowered<Vec<Stmt>> {
        let target = self.variable_place(frame, &assignment.target)?;
        let span = assignment.span();
        let call = UserCall::Function {
            call: func,
            function,
        };
        let (args, stores) = self.call_with_stores(frame, call, &span)?;
        let result = args.result.clone().ok_or(Unlowered::NotYet)?;
        let mut statements = vec![self.program.call_stmt(args.pou, args.args, span.clone())?];
        statements.extend(stores.unwrap_or_default());
        statements.push(self.program.copy(target, result, span)?);
        Ok(statements)
    }
}
