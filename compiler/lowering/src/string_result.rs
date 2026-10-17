//! Calls of functions whose result is a string, moved out of the statement
//! they are in.
//!
//! A function whose result is a string writes it through its result
//! parameter, a reference to a temporary its caller owns
//! (REQ-LOW-lowering-079), so a call of one is a `Call` statement rather than
//! a value. Where lowering meets such a call in a statement, it lowers it to
//! a `Call` statement passing a new temporary for the result, moved out of
//! the statement, and the call's value to a read of the temporary. The
//! statement lowers to its moved calls, in the order lowering moved them,
//! then itself (REQ-LOW-lowering-179). Each moved call has the statement's
//! span, for the line map (REQ-LOW-lowering-063).
//!
//! Lowering meets a call after the calls in its arguments, so an argument's
//! call is moved first, and calls are moved in the order they run. A
//! statement is lowered so only where running its moved calls first keeps
//! the order the source runs things in (`call_order.rs`,
//! REQ-LOW-lowering-180); any other is not lowered.
//!
//! Not moved yet: a call of a function with outputs, which a call in an
//! expression is not lowered as yet, and a call of a method, whose result
//! the route that reads the AST does not compile when it is a string.

use ironplc_dsl::common::FunctionDeclaration;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, Function};
use ironplc_ir::lowered::{Stmt, StrExpr, VarId};

use crate::call_order::keeps_order;
use crate::function::UserCall;
use crate::variables::BodyFrame;
use crate::Lowering;

/// A lowered node, `None` when lowering does not support it, or something
/// inside it, yet.
type Made<T> = Result<Option<T>, Diagnostic>;

/// The calls moved out of the statement being lowered.
#[derive(Debug, Default)]
pub(crate) struct MovedCalls {
    /// The statement's span, which each moved call has.
    span: SourceSpan,
    /// The moved calls, in the order lowering moved them.
    calls: Vec<Stmt>,
    /// The temporary each of `calls` passes for its result, at its
    /// position.
    temporaries: Vec<VarId>,
}

impl<'a> Lowering<'a> {
    /// The string the call `func`, the expression `expr`, of `function`, a
    /// function whose result is a string, yields: a read of the temporary
    /// the call, moved out of the statement being lowered, writes its result
    /// to. `None` when the call is not lowered yet.
    pub(crate) fn string_result_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Made<StrExpr> {
        // A call is moved only out of a statement being lowered.
        let Some(span) = self.moved.as_ref().map(|moved| moved.span.clone()) else {
            return Ok(None);
        };
        let call = UserCall::Function {
            call: func,
            function,
        };
        let Some(args) = self.call_args_without_outputs(frame, call, &span)? else {
            return Ok(None);
        };
        let Some(result) = args.result else {
            return Ok(None);
        };
        let call = self.program.call_stmt(args.pou, args.args, span)?;
        let Some(moved) = self.moved.as_mut() else {
            return Ok(None);
        };
        moved.calls.push(call);
        moved.temporaries.push(result.root());
        self.program.read_str(result, expr.span()).map(Some)
    }

    /// The statements `lower` lowers a statement whose span is `span` to,
    /// after the calls with a string result moved out of it, where running
    /// them first keeps the order the source runs things in
    /// (REQ-LOW-lowering-179, REQ-LOW-lowering-180). A statement inside it
    /// has calls of its own.
    pub(crate) fn with_moved_calls(
        &mut self,
        span: SourceSpan,
        lower: impl FnOnce(&mut Self) -> Made<Vec<Stmt>>,
    ) -> Made<Vec<Stmt>> {
        let enclosing = self.moved.replace(MovedCalls {
            span,
            calls: Vec::new(),
            temporaries: Vec::new(),
        });
        let lowered = lower(self);
        let moved = std::mem::replace(&mut self.moved, enclosing).unwrap_or_default();
        let Some(lowered) = lowered? else {
            return Ok(None);
        };
        if moved.calls.is_empty() {
            return Ok(Some(lowered));
        }
        let kept = keeps_order(
            self.program.types(),
            self.program.variables(),
            &moved.calls,
            &moved.temporaries,
            &lowered,
        );
        if !kept {
            return Ok(None);
        }
        let mut stmts = moved.calls;
        stmts.extend(lowered);
        Ok(Some(stmts))
    }
}
