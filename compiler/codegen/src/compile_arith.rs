//! The arithmetic operators, in both spellings: the operator expression
//! `a + b` and the function form `ADD(a, b, ...)`.
//!
//! Both spellings compile here so that they cannot diverge. Each asks the
//! analyzer's typed step first: an operand pair with a typed overload on the
//! time and date types (`t1 + t2`, `dt + t`, `d1 - d2`) compiles through the
//! routine the typed call (`ADD_TIME(t1, t2)`) compiles through, which knows
//! the units each type is stored in.
//!
//! A numeric pair computes at the width and signedness of its own result
//! type rather than of the variable it is assigned to: each operand whose
//! width differs is compiled at its own and converted, the operator applies
//! at the result type, and the result is converted to the enclosing
//! operation type. That is ADR-0001's promote-operate-truncate applied to
//! the expression: `INT + REAL` converts the `INT` to `REAL` and adds as
//! `REAL`, and `DINT * DINT` assigned to a `LINT` multiplies at 32 bits. An
//! expression whose result type codegen cannot place (a literal's category,
//! a subrange, an enumeration) compiles at the enclosing operation type as
//! before.
//!
//! The function form folds its inputs from the left the same way. See
//! `specs/design/arithmetic-operator-overloads.md`.

use ironplc_analyzer::{resolve_arithmetic_overload, typed_overload, Overload};
use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{BinaryExpr, Expr, Function, Operator};

use super::compile::{CompileContext, OpType, VarTypeInfo};
use super::compile_call::{collect_positional_args, compile_left_fold, emit_conversion_opcode};
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use super::compile_time_arith::{compile_time_arith, time_arith_for, Operand};
use super::type_info::resolve_type_name;
use crate::emit::Emitter;

/// Compiles the arithmetic operator expression `binary`, leaving the result
/// on the stack.
///
/// A pair with a typed overload compiles through its typed routine. A pair
/// whose result type `result` is numeric computes at that type and converts
/// the result to `op_type`, the operation type of the enclosing expression.
/// Any other pair compiles both operands at `op_type` with the operator's
/// opcode.
pub(crate) fn compile_binary_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    binary: &BinaryExpr,
    result: Option<&TypeName>,
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
    if let Some(natural) = numeric_op_type(result) {
        compile_numeric_step(
            emitter,
            ctx,
            &binary.op,
            Operand::Expr(&binary.left),
            &binary.right,
            natural,
        )?;
        convert(emitter, natural, op_type);
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
/// When the first two inputs have a typed overload, every step compiles
/// through its typed routine, the left operand of each step after the first
/// being the previous step's result on the stack. When every step has a
/// numeric result type, each step computes at its own, as the operator
/// expression does. Otherwise every step compiles with the operator's opcode
/// at `op_type`.
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
    if let Some(steps) = numeric_steps(ctx, op, &args) {
        return compile_numeric_fold(emitter, ctx, op, &args, &steps, op_type);
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

/// Returns the operation type of each step of a numeric fold over `args`:
/// the result type of the numeric overload that applies to the accumulated
/// result and the next input. `None` unless there are two or more inputs and
/// every step has a numeric result type codegen can place.
fn numeric_steps(ctx: &CompileContext, op: &Operator, args: &[&Expr]) -> Option<Vec<OpType>> {
    let (first, rest) = args.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let mut accumulated = first.resolved_type.clone();
    let mut steps = Vec::with_capacity(rest.len());
    for arg in rest {
        let overload = resolve_arithmetic_overload(
            op,
            accumulated.as_ref(),
            arg.resolved_type.as_ref(),
            &ctx.compiler_options,
        )?;
        let Overload::Numeric { result } = overload else {
            return None;
        };
        steps.push(numeric_op_type(Some(&result))?);
        accumulated = Some(result);
    }
    Some(steps)
}

/// Compiles a numeric fold, each step at its operation type in `steps`, and
/// converts the result to `op_type`.
fn compile_numeric_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: &Operator,
    args: &[&Expr],
    steps: &[OpType],
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let Some((first, rest)) = args.split_first() else {
        return Ok(());
    };
    let mut left = Operand::Expr(first);
    for (arg, natural) in rest.iter().zip(steps) {
        compile_numeric_step(emitter, ctx, op, left, arg, *natural)?;
        left = Operand::Stack(*natural);
    }
    if let Some(last) = steps.last() {
        convert(emitter, *last, op_type);
    }
    Ok(())
}

/// Compiles one numeric step at `natural`, the operation type of its result:
/// the left operand (compiled, or already on the stack) and `right`, each
/// converted to `natural` when its own width differs, then the operator.
fn compile_numeric_step(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: &Operator,
    left: Operand<'_>,
    right: &Expr,
    natural: OpType,
) -> Result<(), Diagnostic> {
    match left {
        Operand::Expr(expr) => compile_at(emitter, ctx, expr, natural)?,
        Operand::Stack(own) => convert(emitter, own, natural),
    }
    compile_at(emitter, ctx, right, natural)?;
    emit_arithmetic_op(emitter, op, natural);
    Ok(())
}

/// Compiles `operand` for an operation at `target`: at its own operation
/// type followed by a conversion when its width differs, and at `target`
/// otherwise (a literal takes the operation's type, as ADR-0028 types it).
fn compile_at(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    operand: &Expr,
    target: OpType,
) -> Result<(), Diagnostic> {
    match numeric_op_type(operand.resolved_type.as_ref()) {
        Some(own) if own.0 != target.0 => {
            compile_expr(emitter, ctx, operand, own)?;
            convert(emitter, own, target);
            Ok(())
        }
        _ => compile_expr(emitter, ctx, operand, target),
    }
}

/// Emits the conversion of the value on the stack from `from` to `to`, or
/// nothing when the two share an operation width.
fn convert(emitter: &mut Emitter, from: OpType, to: OpType) {
    let info = |(op_width, signedness): OpType| VarTypeInfo {
        op_width,
        signedness,
        storage_bits: 0,
    };
    emit_conversion_opcode(emitter, &info(from), &info(to));
}

/// Returns the operation type of `type_name` when it is a concrete
/// elementary numeric or bit-string type, the types the numeric overload
/// computes at, and `None` for anything else: a literal's category, a
/// subrange, an enumeration, a temporal type, or no type.
fn numeric_op_type(type_name: Option<&TypeName>) -> Option<OpType> {
    let type_name = type_name?;
    let elementary = ElementaryTypeName::try_from(&type_name.name).ok()?;
    let numeric = GenericTypeName::AnyNum.is_compatible_with(&elementary)
        || matches!(
            elementary,
            ElementaryTypeName::BYTE
                | ElementaryTypeName::WORD
                | ElementaryTypeName::DWORD
                | ElementaryTypeName::LWORD
        );
    if !numeric {
        return None;
    }
    let info = resolve_type_name(&type_name.name)?;
    Some((info.op_width, info.signedness))
}
