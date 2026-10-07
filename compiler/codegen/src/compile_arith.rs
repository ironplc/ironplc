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
//! type, and a function form of three or more inputs arrives as the calls it
//! folds to. Both compile here as the pair of operands they are. An
//! expression whose result type codegen cannot place (a literal's category, a
//! subrange, an enumeration) compiles at the enclosing operation type.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_analyzer::{typed_overload, Intrinsic, Overload};
use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{BinaryExpr, Expr, Function, Operator};

use super::call_args::collect_positional_args;
use super::compile::{CompileContext, OpType, VarTypeInfo};
use super::compile_call::{compile_left_fold, emit_conversion_opcode};
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use super::compile_time_arith::{compile_time_arith, time_arith_for, Operand};
use super::type_info::{expr_operand_name, resolve_type_name};
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
    if let Some(left) = expr_operand_name(ctx, &binary.left) {
        if let Some((name, _)) = typed_step(ctx, &binary.op, &left, &binary.right) {
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
        return compile_numeric_pair(
            emitter,
            ctx,
            &binary.op,
            (&binary.left, &binary.right),
            natural,
            op_type,
        );
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
    result: Option<&TypeName>,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    if let [first, second, rest @ ..] = args.as_slice() {
        if let Some(left) = expr_operand_name(ctx, first) {
            if let Some((name, result)) = typed_step(ctx, op, &left, second) {
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
    if let [left, right] = args.as_slice() {
        if let Some(natural) = numeric_pair_op_type(ctx, result, left, right) {
            return compile_numeric_pair(emitter, ctx, op, (left, right), natural, op_type);
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
        let Some((name, result)) = typed_step(ctx, op, &accumulated, arg) else {
            return Err(Diagnostic::todo_with_span(span));
        };
        let Some(natural) = resolve_type_name(&accumulated.name) else {
            return Err(Diagnostic::internal_error_at(Label::span(
                span,
                "Typed overload result is not an elementary type",
            )));
        };
        let left = Operand::Stack((natural.op_width, natural.signedness));
        compile_typed(emitter, ctx, name, left, arg, span.clone())?;
        accumulated = result;
    }
    Ok(())
}

/// Returns the typed overload of `op` on `left` and the operand `right`, as
/// the typed name and its result type, or `None` when the pair has none.
fn typed_step(
    ctx: &CompileContext,
    op: &Operator,
    left: &TypeName,
    right: &Expr,
) -> Option<(&'static str, TypeName)> {
    match typed_overload(op, left, &expr_operand_name(ctx, right)?)? {
        Overload::Typed { name, result } => Some((name, result)),
        Overload::Unchecked { .. } | Overload::Numeric { .. } => None,
    }
}

/// Compiles the typed overload `name` over `left` and `right` through the
/// routine of the time function its signature names.
fn compile_typed(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    name: &str,
    left: Operand<'_>,
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

/// Returns the operation type the two inputs of a call compile at: that of
/// the call's `result` when it is numeric and each input, which the analyzer
/// converted to it where its width differed, is a numeric type of that
/// width. `None` for any other pair, which compiles at the enclosing
/// operation type.
fn numeric_pair_op_type(
    ctx: &CompileContext,
    result: Option<&TypeName>,
    left: &Expr,
    right: &Expr,
) -> Option<OpType> {
    let natural = numeric_op_type(result)?;
    let at_natural = |input: &Expr| {
        numeric_op_type(expr_operand_name(ctx, input).as_ref())
            .is_some_and(|own| own.0 == natural.0)
    };
    (at_natural(left) && at_natural(right)).then_some(natural)
}

/// Compiles the two operands of a numeric operation at `natural`, the
/// operation type of its result, then the operator, and converts the result
/// to `op_type`.
fn compile_numeric_pair(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: &Operator,
    (left, right): (&Expr, &Expr),
    natural: OpType,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    compile_expr(emitter, ctx, left, natural)?;
    compile_expr(emitter, ctx, right, natural)?;
    emit_arithmetic_op(emitter, op, natural);
    convert(emitter, natural, op_type);
    Ok(())
}

/// Compiles, by `compile`, an operation whose result has the type of its
/// operand, `result`, then converts the result to `op_type`, the operation
/// type of the enclosing expression.
///
/// A negation, `NOT`, a numeric function of one input, `MOVE` and a shift or
/// rotate compute at their own type when it is numeric, as an arithmetic
/// operation does: `SHL` of a `DWORD` assigned to an `LWORD` shifts the 32
/// bits of the `DWORD` and widens the result. Any other, such as `NOT` of a
/// `BOOL`, computes at `op_type`.
pub(crate) fn compile_at_operand_type(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    result: Option<&TypeName>,
    op_type: OpType,
    compile: impl FnOnce(&mut Emitter, &mut CompileContext, OpType) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let at = numeric_op_type(result).unwrap_or(op_type);
    compile_at(emitter, ctx, at, op_type, compile)
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
    convert(emitter, at, op_type);
    Ok(())
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
