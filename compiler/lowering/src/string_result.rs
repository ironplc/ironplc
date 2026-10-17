//! Calls of functions whose result is a string.
//!
//! A function whose result is a string writes it through its result
//! parameter, a reference to a temporary its caller owns
//! (REQ-LOW-lowering-079), so a call of one is not a value. Where lowering
//! meets such a call, it lowers it to a string in-place call: the call,
//! passing a new temporary for the result, then the stores of its outputs,
//! if any, whose string is the one the function wrote to the temporary
//! (REQ-LOW-lowering-179). The expression that holds it runs it where the
//! source evaluates the call.
//!
//! Not lowered yet: a call of a method, whose result the route that reads
//! the AST does not compile when it is a string.

use ironplc_dsl::common::FunctionDeclaration;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, Function};
use ironplc_ir::lowered::StrExpr;

use crate::function::UserCall;
use crate::variables::BodyFrame;
use crate::Lowering;

impl<'a> Lowering<'a> {
    /// The string the call `func`, the expression `expr`, of `function`, a
    /// function whose result is a string, yields: an in-place call of it.
    /// `None` when the call is not lowered yet.
    pub(crate) fn string_result_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Result<Option<StrExpr>, Diagnostic> {
        let span = expr.span();
        let call = UserCall::Function {
            call: func,
            function,
        };
        let Some((args, stores)) = self.call_with_stores(frame, call, &span)? else {
            return Ok(None);
        };
        if args.result.is_none() {
            return Ok(None);
        }
        self.program
            .str_call_in_place(args.pou, args.args, stores.unwrap_or_default(), span)
            .map(Some)
    }
}
