//! The comparisons `=`, `<>`, `<`, `<=`, `>`, `>=`, in both spellings: the
//! operator expression `a < b` and the function form `LT(a, b)`.
//!
//! A comparison's result is `BOOL`, so the operation type it computes at
//! comes from its operands. The analyzer has already settled it: it gave an
//! untyped literal operand the operand type, and wrapped an operand of
//! another type in an `ImplicitConversion` (ADR-0056), so both operands have
//! the operand type and codegen compiles the comparison at the left one's.
//! See `specs/design/comparison-operand-type.md`.

use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareOp, Expr};

use super::compile::{CompileContext, OpType};
use super::compile_expr::{compile_expr, emit_compare_op, expr_is_string, op_type_from_expr};
use super::compile_string::compile_string_compare;
use crate::emit::Emitter;

/// Compiles the comparison `op` of `left` and `right`, leaving the `BOOL`
/// result on the stack. Strings compare through `compile_string_compare`.
///
/// `op_type` is the operation type of the enclosing expression. It is used
/// only when neither operand has a type codegen can place, such as a direct
/// address the analyzer does not type yet.
pub(crate) fn compile_comparison(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: &CompareOp,
    left: &Expr,
    right: &Expr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    // Strings live in the data region, not on the operand stack, and
    // compare through their own builtin.
    if expr_is_string(ctx, left) {
        return compile_string_compare(emitter, ctx, op, left, right);
    }
    let operand_type = op_type_from_expr(ctx, left)
        .or_else(|| op_type_from_expr(ctx, right))
        .unwrap_or(op_type);
    compile_expr(emitter, ctx, left, operand_type)?;
    compile_expr(emitter, ctx, right, operand_type)?;
    emit_compare_op(emitter, op, operand_type);
    Ok(())
}
