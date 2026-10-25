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
//! Not lowered yet: a call of a method whose result is a string.

use ironplc_dsl::common::FunctionDeclaration;
use ironplc_dsl::core::Located;
use ironplc_dsl::textual::{Expr as AstExpr, Function};
use ironplc_ir::lowered::StrExpr;

use crate::function::UserCall;
use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

impl<'a> Lowering<'a> {
    /// The string the call `func`, the expression `expr`, of `function`, a
    /// function whose result is a string, yields: an in-place call of it. A
    /// call of a function whose result is a scalar is a value (`function.rs`),
    /// not a string.
    pub(crate) fn string_result_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Lowered<StrExpr> {
        let span = expr.span();
        let call = UserCall::Function {
            call: func,
            function,
        };
        let (args, stores) = self.call_with_stores(frame, call, &span)?;
        if args.result.is_none() {
            return Err(Unlowered::NotYet);
        }
        Ok(self
            .program
            .str_call_in_place(args.pou, args.args, stores.unwrap_or_default(), span)?)
    }
}
