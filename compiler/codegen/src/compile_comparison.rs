//! The comparisons `=`, `<>`, `<`, `<=`, `>`, `>=`, in both spellings: the
//! operator expression `a < b` and the function form `LT(a, b)`.
//!
//! A comparison's result is `BOOL`, so the operation type it computes at
//! comes from its operands: the type one of them widens to, whichever side it
//! is on, as the analyzer's `comparison_operand_type` answers. Each operand
//! is compiled at its own type and converted, so a narrower operand is
//! widened by its own signedness -- an unsigned integer, a bit string or a
//! date (ADR-0025) is zero-extended, a signed integer or a `TIME` (ADR-0021)
//! is sign-extended, an integer becomes a real -- rather than truncating the
//! wider one. See `specs/design/comparison-operand-type.md`.

use ironplc_analyzer::comparison_operand_type;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareOp, Expr};

use super::compile::{CompileContext, OpType};
use super::compile_arith::convert;
use super::compile_expr::{
    compile_expr, concrete_op_type_from_expr, emit_compare_op, expr_is_string, op_type_from_expr,
};
use super::compile_string::compile_string_compare;
use super::type_info::{expr_operand_name, resolve_type_name};
use crate::emit::Emitter;

/// Compiles the comparison `op` of `left` and `right`, leaving the `BOOL`
/// result on the stack. Strings compare through `compile_string_compare`.
///
/// `op_type` is the operation type of the enclosing expression. It is used
/// only when neither operand has a type codegen can place.
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
    let operand_type = comparison_op_type(ctx, left, right, op_type);
    compile_operand(emitter, ctx, left, operand_type)?;
    compile_operand(emitter, ctx, right, operand_type)?;
    emit_compare_op(emitter, op, operand_type);
    Ok(())
}

/// Returns the operation type a comparison of `left` and `right` computes
/// at.
///
/// It is the operation type of the analyzer's operand type for the pair. A
/// pair without one -- an operand the analyzer does not judge (a subrange,
/// an enumeration, a reference); a pair of types it judges and neither of
/// which widens to the other is reported as P4049 before codegen -- computes
/// at the concrete left operand's type, else the concrete right operand's,
/// as every comparison did before the operand type was resolved.
fn comparison_op_type(ctx: &CompileContext, left: &Expr, right: &Expr, op_type: OpType) -> OpType {
    comparison_operand_type(
        expr_operand_name(ctx, left).as_ref(),
        expr_operand_name(ctx, right).as_ref(),
        &ctx.compiler_options,
    )
    .and_then(|common| resolve_type_name(&common.name))
    .map(|info| (info.op_width, info.signedness))
    .or_else(|| concrete_op_type_from_expr(ctx, left))
    .or_else(|| concrete_op_type_from_expr(ctx, right))
    .or_else(|| op_type_from_expr(ctx, left))
    .unwrap_or(op_type)
}

/// Compiles an operand of a comparison computed at `target`: at its own
/// operation type followed by a conversion when its width differs, and at
/// `target` otherwise (a literal takes the comparison's type).
fn compile_operand(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    operand: &Expr,
    target: OpType,
) -> Result<(), Diagnostic> {
    match concrete_op_type_from_expr(ctx, operand) {
        Some(own) if own.0 != target.0 => {
            compile_expr(emitter, ctx, operand, own)?;
            convert(emitter, own, target);
            Ok(())
        }
        _ => compile_expr(emitter, ctx, operand, target),
    }
}

/// Returns `true` for the comparisons `=`, `<>`, `<`, `<=`, `>` and `>=`,
/// and `false` for the logical and bitwise operators that share
/// `CompareOp`.
pub(crate) fn is_comparison(op: &CompareOp) -> bool {
    match op {
        CompareOp::Eq
        | CompareOp::Ne
        | CompareOp::Lt
        | CompareOp::Gt
        | CompareOp::LtEq
        | CompareOp::GtEq => true,
        CompareOp::And
        | CompareOp::Or
        | CompareOp::Xor
        | CompareOp::AndThen
        | CompareOp::OrElse => false,
    }
}
