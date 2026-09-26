//! The arithmetic operators, in both spellings: the operator expression
//! `a + b` and the function form `ADD(a, b, ...)`.
//!
//! Both spellings compile here so that they cannot diverge. A pair of
//! operands that selects a typed overload on the time and date types
//! (IEC 61131-3 Table 30, `ADD_TIME`, `SUB_DATE_DATE`, ...) compiles as
//! that typed function, whose routine knows the units the operands are
//! stored in; the analyzer's [`typed_overload`] says which. Any other pair
//! compiles as the numeric operator at the enclosing operation type, and
//! the function form folds its inputs from the left with the same opcode
//! the operator expression emits.

use ironplc_analyzer::{typed_overload, Overload};
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{BinaryExpr, Expr, Function, Operator};

use super::compile::{CompileContext, OpType};
use super::compile_call::collect_positional_args;
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use super::compile_time_arith::{compile_typed_overload, widen_stack_value, Operand};
use crate::emit::Emitter;

/// Compiles the arithmetic operator expression `binary`, leaving the result
/// on the stack.
///
/// A Table 30 pair compiles as its typed overload. Otherwise both operands
/// compile at `op_type`, the operation type of the enclosing expression.
pub(crate) fn compile_binary_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    binary: &BinaryExpr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let left_type = binary.left.resolved_type.as_ref();
    if let Some((name, result)) = typed_overload_of(&binary.op, left_type, &binary.right) {
        let from = compile_typed_overload(
            emitter,
            ctx,
            name,
            Operand::Expr(&binary.left),
            &binary.right,
            &result,
        )?;
        widen_stack_value(emitter, from, op_type);
        return Ok(());
    }
    compile_expr(emitter, ctx, &binary.left, op_type)?;
    compile_expr(emitter, ctx, &binary.right, op_type)?;
    emit_arithmetic_op(emitter, &binary.op, op_type);
    Ok(())
}

/// Compiles a call to the function form of the arithmetic operator `op`,
/// folding its inputs from the left: `ADD(a, b, c)` is `(a + b) + c`.
///
/// Each step asks the typed step with the accumulated result type on the
/// left, as the resolver does, so `ADD(t1, t2, t3)` is two `ADD_TIME`
/// steps and compiles as `t1 + t2 + t3` does.
pub(crate) fn compile_arith_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op: &Operator,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    let [first, rest @ ..] = args.as_slice() else {
        return Err(Diagnostic::todo_with_span(func.name.span()));
    };
    if rest.is_empty() {
        return Err(Diagnostic::todo_with_span(func.name.span()));
    }
    // The accumulated type on the left of the next step, and the operation
    // type of the value on the stack once the first step has run.
    let mut acc: Option<TypeName> = first.resolved_type.clone();
    let mut on_stack: Option<OpType> = None;
    for arg in rest {
        let left = match on_stack {
            None => Operand::Expr(first),
            Some(from) => Operand::Stack(from),
        };
        if let Some((name, result)) = typed_overload_of(op, acc.as_ref(), arg) {
            let from = compile_typed_overload(emitter, ctx, name, left, arg, &result)?;
            on_stack = Some(from);
            acc = Some(result);
            continue;
        }
        match left {
            Operand::Expr(expr) => compile_expr(emitter, ctx, expr, op_type)?,
            Operand::Stack(from) => widen_stack_value(emitter, from, op_type),
        }
        compile_expr(emitter, ctx, arg, op_type)?;
        emit_arithmetic_op(emitter, op, op_type);
        on_stack = Some(op_type);
    }
    if let Some(from) = on_stack {
        widen_stack_value(emitter, from, op_type);
    }
    Ok(())
}

/// Returns the typed overload of `op` for a left operand of type `left`
/// and the right operand expression `right`, as the typed function's name
/// and result type, or `None` when the pair is not a Table 30 pair.
fn typed_overload_of(
    op: &Operator,
    left: Option<&TypeName>,
    right: &Expr,
) -> Option<(&'static str, TypeName)> {
    match typed_overload(op, left?, right.resolved_type.as_ref()?) {
        Some(Overload::Typed { name, result }) => Some((name, result)),
        _ => None,
    }
}
