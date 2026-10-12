//! A subscript of an array access, whatever it is compiled from.
//!
//! The flat index of an array element is computed from its subscripts in one
//! way (`emit_flat_index`), whether they come from the AST or from the lowered
//! program. [`Subscript`] is what that computation needs of one subscript.

use ironplc_dsl::common::ConstantKind;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, UnaryOp};

use super::compile::{CompileContext, OpWidth};
use super::compile_expr::{compile_expr, op_type};
use crate::emit::Emitter;

/// One subscript of an array access.
pub(crate) trait Subscript {
    /// The subscript's value when it is an integer literal, so the flat
    /// index can be computed at compile time.
    fn literal(&self) -> Option<i32>;

    /// Pushes the subscript's value, a `DINT`.
    fn emit(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic>;
}

impl Subscript for &Expr {
    fn literal(&self) -> Option<i32> {
        integer_literal(self)
    }

    /// Compiles the subscript at its own type, which the analyzer converted
    /// to `DINT` where its width differed.
    fn emit(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
        let own = op_type(ctx, self)?;
        if own.0 != OpWidth::W32 {
            return Err(Diagnostic::internal_error_at(Label::span(
                self.span(),
                "Array subscript of a width other than DINT's, and the analyzer recorded no conversion",
            )));
        }
        compile_expr(emitter, ctx, self)
    }
}

/// Extracts an i32 value from an expression if it is a literal integer.
/// Returns `None` for any non-literal expression.
fn integer_literal(expr: &Expr) -> Option<i32> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            let unsigned = i32::try_from(lit.value.value.value).ok()?;
            if lit.value.is_neg {
                unsigned.checked_neg()
            } else {
                Some(unsigned)
            }
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => match &unary.term.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
                let val = i32::try_from(lit.value.value.value).ok()?;
                if lit.value.is_neg {
                    Some(val)
                } else {
                    val.checked_neg()
                }
            }
            _ => None,
        },
        _ => None,
    }
}
