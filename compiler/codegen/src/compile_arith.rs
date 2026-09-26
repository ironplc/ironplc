//! The arithmetic operators, in both spellings: the operator expression
//! `a + b` and the function form `ADD(a, b, ...)`.
//!
//! Both spellings compile here so that they cannot diverge. Each asks the
//! analyzer's typed step first: an operand pair with a typed overload on the
//! time and date types (`t1 + t2`, `dt + t`, `d1 - d2`) compiles through the
//! routine the typed call (`ADD_TIME(t1, t2)`) compiles through, which knows
//! the units each type is stored in. Any other pair compiles with the
//! operator's opcode. The function form folds its inputs from the left the
//! same way. See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_analyzer::{typed_overload, Overload};
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{BinaryExpr, Expr, Function, Operator};

use super::compile::{CompileContext, OpType};
use super::compile_call::{collect_positional_args, compile_left_fold};
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use super::compile_time_arith::{compile_time_arith, time_arith_for, Operand};
use super::type_info::resolve_type_name;
use crate::emit::Emitter;

/// Compiles the arithmetic operator expression `binary`, leaving the result
/// on the stack.
///
/// A pair with a typed overload compiles through its typed routine. Any
/// other pair compiles both operands at `op_type`, the operation type of the
/// enclosing expression, with the operator's opcode.
pub(crate) fn compile_binary_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    binary: &BinaryExpr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    if let Some(left) = &binary.left.resolved_type {
        if let Some((name, _)) = typed_step(&binary.op, left, &binary.right) {
            let span = binary.left.span();
            return compile_typed(
                emitter,
                ctx,
                name,
                Operand::Expr(&binary.left),
                &binary.right,
                span,
            );
        }
    }
    compile_expr(emitter, ctx, &binary.left, op_type)?;
    compile_expr(emitter, ctx, &binary.right, op_type)?;
    emit_arithmetic_op(emitter, &binary.op, op_type);
    Ok(())
}

/// Compiles a call to the function form of the arithmetic operator `op`,
/// folding its inputs from the left: `ADD(a, b, c)` is `(a + b) + c`.
///
/// When the first two inputs have a typed overload, every step compiles
/// through its typed routine, the left operand of each step after the first
/// being the previous step's result on the stack. Otherwise every step
/// compiles with the operator's opcode at `op_type`.
pub(crate) fn compile_arith_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op: &Operator,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    if let [first, second, rest @ ..] = args.as_slice() {
        if let Some(left) = &first.resolved_type {
            if let Some((name, result)) = typed_step(op, left, second) {
                let span = func.name.span();
                compile_typed(
                    emitter,
                    ctx,
                    name,
                    Operand::Expr(first),
                    second,
                    span.clone(),
                )?;
                return compile_typed_rest(emitter, ctx, op, result, rest, span);
            }
        }
    }
    compile_left_fold(emitter, ctx, func, op_type, |emitter, op_type| {
        emit_arithmetic_op(emitter, op, op_type)
    })
}

/// Compiles the steps of a typed fold after the first, whose result of type
/// `accumulated` is on the stack.
fn compile_typed_rest(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: &Operator,
    mut accumulated: TypeName,
    rest: &[&Expr],
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    for arg in rest {
        // The analyzer resolves every step of a fold; a step without a
        // typed overload after one with it is a pair it rejected.
        let Some((name, result)) = typed_step(op, &accumulated, arg) else {
            return Err(Diagnostic::todo_with_span(span));
        };
        let Some(natural) = resolve_type_name(&accumulated.name) else {
            return Err(Diagnostic::todo_with_span(span));
        };
        let left = Operand::Stack((natural.op_width, natural.signedness));
        compile_typed(emitter, ctx, name, left, arg, span.clone())?;
        accumulated = result;
    }
    Ok(())
}

/// Returns the typed overload of `op` on `left` and the operand `right`, as
/// the typed name and its result type, or `None` when the pair has none.
fn typed_step(op: &Operator, left: &TypeName, right: &Expr) -> Option<(&'static str, TypeName)> {
    match typed_overload(op, left, right.resolved_type.as_ref()?)? {
        Overload::Typed { name, result } => Some((name, result)),
        Overload::Unchecked { .. } | Overload::Numeric { .. } => None,
    }
}

/// Compiles the typed overload `name` over `left` and `right` through its
/// routine.
fn compile_typed(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    name: &str,
    left: Operand<'_>,
    right: &Expr,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // Every typed name the analyzer answers with has a routine; a test pins
    // it for both widths of every overload.
    let Some((arith, width)) = time_arith_for(&name.to_ascii_lowercase()) else {
        return Err(Diagnostic::internal_error_at(Label::span(
            span,
            format!("No routine for the typed overload {name}"),
        )));
    };
    compile_time_arith(emitter, ctx, arith, width, left, right)
}
