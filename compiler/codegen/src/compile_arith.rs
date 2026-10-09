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
//! type rather than of the variable it is assigned to, and the result is
//! converted to the enclosing operation type. That is ADR-0001's
//! promote-operate-truncate applied to the expression: `INT + REAL` adds as
//! `REAL`, and `DINT * DINT` assigned to a `LINT` multiplies at 32 bits.
//!
//! The analyzer decided which operand converts (ADR-0056): an operand of
//! another width arrives wrapped in an `ImplicitConversion` to the result
//! type, a function form of three or more inputs whose steps compute at
//! different types arrives as the calls it folds to, and an operand of a typed
//! pair is converted to the width its routine computes at. Every operation
//! compiles at its own result type, a subrange's being its base type. One
//! without a typed overload or a numeric result type, or with an input of
//! another width, is one the analyzer did not resolve, and is an internal
//! error rather than compiled at the type of its context.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_analyzer::{typed_overload, Intrinsic};
use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{BinaryExpr, Expr, Function, Operator};

use super::call_args::collect_positional_args;
use super::compile::{CompileContext, OpType, VarTypeInfo};
use super::compile_call::{compile_left_fold, emit_conversion_opcode};
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use super::compile_time_arith::{compile_time_arith, time_arith_for};
use super::type_info::{expr_operand_name, resolve_type_name};
use crate::emit::Emitter;

/// Compiles `expr`, the arithmetic operator expression `binary`, leaving the
/// result on the stack.
///
/// A pair with a typed overload compiles through its typed routine. A pair
/// whose result type is numeric computes at that type and converts the
/// result to `op_type`, the operation type of the enclosing expression. The
/// analyzer resolves every pair to one or the other.
pub(crate) fn compile_binary_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &Expr,
    binary: &BinaryExpr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    if let Some(name) = typed_step(ctx, &binary.op, &binary.left, &binary.right) {
        let span = binary.left.span();
        return compile_typed(emitter, ctx, name, &binary.left, &binary.right, span);
    }
    let operands = [&binary.left, &binary.right];
    let Some(natural) =
        numeric_inputs_op_type(ctx, expr_operand_name(ctx, expr).as_ref(), &operands)
    else {
        return Err(unresolved_operation(expr.span()));
    };
    compile_at(emitter, ctx, natural, op_type, |emitter, ctx, at| {
        compile_expr(emitter, ctx, &binary.left, at)?;
        compile_expr(emitter, ctx, &binary.right, at)?;
        emit_arithmetic_op(emitter, &binary.op, at);
        Ok(())
    })
}

/// Compiles a call to the function form of the arithmetic operator `op`,
/// folding its inputs from the left: `ADD(a, b, c)` is `(a + b) + c`.
///
/// A pair with a typed overload compiles through its typed routine; the
/// analyzer writes a typed fold of three or more inputs as the two-input calls
/// it folds to. Otherwise every step computes at the call's numeric result
/// type `result`, at whose width the analyzer placed every input, and the
/// result is converted to `op_type`. A fold whose steps compute at different
/// types arrives as the two-input calls it folds to, each its own call.
pub(crate) fn compile_arith_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op: &Operator,
    result: Option<&TypeName>,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    if let [first, second, rest @ ..] = args.as_slice() {
        if let Some(name) = typed_step(ctx, op, first, second) {
            let span = func.name.span();
            if !rest.is_empty() {
                return Err(Diagnostic::internal_error_at(Label::span(
                    span,
                    "Typed fold the analyzer did not write as the calls it folds to",
                )));
            }
            return compile_typed(emitter, ctx, name, first, second, span);
        }
    }
    let Some(natural) = numeric_inputs_op_type(ctx, result, &args) else {
        return Err(unresolved_operation(func.name.span()));
    };
    compile_at(emitter, ctx, natural, op_type, |emitter, ctx, at| {
        compile_left_fold(emitter, ctx, func, at, |emitter, at| {
            emit_arithmetic_op(emitter, op, at)
        })
    })
}

/// Returns the name of the typed overload of `op` on the operands `left` and
/// `right`, or `None` when the pair has none.
fn typed_step(
    ctx: &CompileContext,
    op: &Operator,
    left: &Expr,
    right: &Expr,
) -> Option<&'static str> {
    let left = expr_operand_name(ctx, left)?;
    let right = expr_operand_name(ctx, right)?;
    typed_overload(op, &left, &right).map(|typed| typed.name)
}

/// Compiles the typed overload `name` over `left` and `right` through the
/// routine of the time function its signature names.
fn compile_typed(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    name: &str,
    left: &Expr,
    right: &Expr,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // Every typed name the analyzer answers with is registered as a time
    // function; a test pins it for both widths of every overload.
    let Some(Intrinsic::Time { function, long }) = ctx.intrinsics.get(&Id::from(name)).cloned()
    else {
        return Err(Diagnostic::internal_error_at(Label::span(
            span,
            format!("No time function for the typed overload {name}"),
        )));
    };
    let (arith, width) = time_arith_for(function, long);
    compile_time_arith(emitter, ctx, arith, width, left, right)
}

/// Returns the operation type the inputs of an arithmetic operation compile
/// at: that of its `result` when it is numeric and each input, which the
/// analyzer converted to it where its width differed, is a numeric type of
/// that width. `None` for any other operation, which the analyzer did not
/// resolve.
fn numeric_inputs_op_type(
    ctx: &CompileContext,
    result: Option<&TypeName>,
    inputs: &[&Expr],
) -> Option<OpType> {
    let natural = numeric_op_type(result)?;
    let at_natural = |input: &&Expr| {
        numeric_op_type(expr_operand_name(ctx, input).as_ref())
            .is_some_and(|own| own.0 == natural.0)
    };
    inputs.iter().all(at_natural).then_some(natural)
}

/// The internal error for an arithmetic operation the analyzer resolved to
/// neither a typed overload nor a numeric type its inputs are placed at.
fn unresolved_operation(span: SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span,
        "Arithmetic operation has no typed overload or numeric type its inputs are converted to",
    ))
}

/// Compiles, by `compile`, an operation at the operation type `at`, then
/// converts the result to `op_type`, the operation type of the enclosing
/// expression.
pub(crate) fn compile_at(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    at: OpType,
    op_type: OpType,
    compile: impl FnOnce(&mut Emitter, &mut CompileContext, OpType) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    compile(emitter, ctx, at)?;
    convert_to_context(emitter, at, op_type);
    Ok(())
}

/// Converts the value on the stack, computed at its own operation type
/// `own`, to `context`, the operation type of the expression that encloses
/// it.
///
/// This is the one place codegen converts a value without the analyzer
/// having recorded the conversion (ADR-0056). Every other conversion is
/// one the analyzer recorded in an `ImplicitConversion`, or one the
/// program wrote, such as `INT_TO_REAL`.
pub(crate) fn convert_to_context(emitter: &mut Emitter, own: OpType, context: OpType) {
    convert(emitter, own, context);
}

/// Emits the conversion of the value on the stack from `from` to `to`, or
/// nothing when the two share an operation width.
pub(crate) fn convert(emitter: &mut Emitter, from: OpType, to: OpType) {
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
pub(crate) fn numeric_op_type(type_name: Option<&TypeName>) -> Option<OpType> {
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
