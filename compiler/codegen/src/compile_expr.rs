//! Expression compilation for IEC 61131-3 code generation.
//!
//! Contains expression dispatch, constant compilation, variable reads,
//! and typed opcode emission helpers. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_analyzer::SemanticType;
use ironplc_container::{opcode, VarIndex};
use ironplc_dsl::common::{Boolean, ConstantKind, SignedInteger};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CompareExpr, CompareOp, Expr, ExprKind, ExprType, Operator, SymbolicVariableKind, UnaryOp,
    Variable,
};
use ironplc_problems::Problem;
use paste::paste;

use super::compile::{
    encode_string_literal, CompileContext, OpType, OpWidth, Signedness, VarTypeInfo,
    DEFAULT_OP_TYPE, NARROW_CHAR_WIDTH,
};
use super::compile_arith::{compile_at_operand_type, compile_binary_arith};
use super::compile_call::compile_function_call;
use super::compile_comparison::compile_comparison;
use super::compile_method::compile_method_call_expression;
use super::compile_partial_access::{compile_partial_access_read, PartialAccess};
use super::compile_short_circuit::{compile_short_circuit, ShortCircuitOp};
use super::type_info::{expr_operand_name, expr_representation, expr_type_info};
use crate::emit::Emitter;

/// Returns the operation type of an expression's value, from its
/// `expr_type`.
///
/// The analyzer must have given the expression a type this backend operates
/// on. Anything else -- an array or a structure, say -- is a compiler bug.
pub(crate) fn op_type(ctx: &CompileContext, expr: &Expr) -> Result<OpType, Diagnostic> {
    let info = expr_type_info(ctx, expr).ok_or_else(|| unresolved_expr_type(expr))?;
    Ok((info.op_width, info.signedness))
}

/// Returns the operation type of an expression's value, if it has one.
///
/// Unlike [`op_type`] this returns `None` instead of an error when the
/// type is missing or not one this backend operates on, making it safe to
/// use as a best-effort fallback.
pub(crate) fn op_type_from_expr(ctx: &CompileContext, expr: &Expr) -> Option<OpType> {
    let info = expr_type_info(ctx, expr)?;
    Some((info.op_width, info.signedness))
}

/// Returns the operation type only when the expression has a type, stated
/// or inferred, not an untyped literal's generic category.
///
/// Generic types like `ANY_INT` map to a signed default (`DINT`) which is
/// wrong when the other operand is unsigned (e.g. `DWORD`). Returning
/// `None` for generic types lets callers prefer a concrete type from
/// another operand.
pub(crate) fn concrete_op_type_from_expr(ctx: &CompileContext, expr: &Expr) -> Option<OpType> {
    if !matches!(
        expr.expr_type,
        Some(ExprType::Concrete(_) | ExprType::Inferred(_))
    ) {
        return None;
    }
    op_type_from_expr(ctx, expr)
}

/// Returns `true` if the expression's value is a BOOL.
pub(crate) fn expr_is_bool(ctx: &CompileContext, expr: &Expr) -> bool {
    matches!(expr_representation(ctx, expr), Some(SemanticType::Bool))
}

/// Returns `true` if the expression's value is a STRING or WSTRING.
pub(crate) fn expr_is_string(ctx: &CompileContext, expr: &Expr) -> bool {
    matches!(
        expr_representation(ctx, expr),
        Some(SemanticType::String { .. })
    )
}

/// Returns the storage bit width of an expression's value.
///
/// The analyzer must have given the expression a type this backend operates
/// on. Anything else is a compiler bug.
pub(crate) fn storage_bits(ctx: &CompileContext, expr: &Expr) -> Result<u8, Diagnostic> {
    let info = expr_type_info(ctx, expr).ok_or_else(|| unresolved_expr_type(expr))?;
    Ok(info.storage_bits)
}

/// Builds the P9999 for an expression whose type the analyzer did not resolve
/// to one codegen knows, pointing at the expression.
///
/// The analyzer leaves `expr_type` empty for constructs it does not type
/// yet (a direct address such as `%QX0.0`, for example), so this is a gap in
/// the compiler rather than an invalid program.
#[track_caller]
pub(crate) fn unresolved_expr_type(expr: &Expr) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(expr.span(), "Expression has no resolved type"))
}

/// Returns the operation type for compiling a condition expression.
///
/// For comparison operators (`>`, `<`, `=`, etc.), returns the type of the
/// left operand, which the analyzer made the comparison's operand type
/// (ADR-0056). For boolean combinations (AND,
/// OR, XOR), recurses into the first operand. For other expressions (bare
/// boolean variables, parenthesized expressions), returns the expression's
/// own resolved type.
pub(crate) fn condition_op_type(ctx: &CompileContext, expr: &Expr) -> Result<OpType, Diagnostic> {
    match &expr.kind {
        ExprKind::Compare(compare) => match compare.op {
            CompareOp::And
            | CompareOp::Or
            | CompareOp::Xor
            | CompareOp::AndThen
            | CompareOp::OrElse => condition_op_type(ctx, &compare.left),
            _ => {
                // String comparisons take a dedicated path in compile_expr
                // that emits an i32 boolean; the operand op_type is unused.
                if expr_is_string(ctx, &compare.left) {
                    return Ok(DEFAULT_OP_TYPE);
                }
                op_type(ctx, &compare.left)
            }
        },
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Not => condition_op_type(ctx, &unary.term),
        ExprKind::Expression(inner) => condition_op_type(ctx, inner),
        _ => op_type(ctx, expr),
    }
}
/// Compiles an expression, leaving the result on the stack.
///
/// The `op_type` determines which width (i32/i64) and signedness to use
/// for arithmetic, comparison, and load/store instructions.
pub(crate) fn compile_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &Expr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        // The analyzer recorded the type a numeric literal is compiled at
        // (ADR-0056). One codegen builds itself, such as a standard function
        // block's member initializer, has none and is stored at the default
        // slot type. Any other literal names its own type and is compiled
        // for the storage its context gives it.
        ExprKind::Const(
            constant @ (ConstantKind::IntegerLiteral(_) | ConstantKind::RealLiteral(_)),
        ) => {
            let own = op_type_from_expr(ctx, expr).unwrap_or(DEFAULT_OP_TYPE);
            compile_constant(emitter, ctx, constant, own)
        }
        ExprKind::Const(constant) => compile_constant(emitter, ctx, constant, op_type),
        // A variable read at a different width is read at its own and
        // converted: loading an INT's slot as a REAL would reinterpret its
        // bits, and loading a UDINT's as a LINT would sign-extend it.
        ExprKind::Variable(variable) => {
            match crate::compile_arith::numeric_op_type(expr_operand_name(ctx, expr).as_ref()) {
                Some(own) if own.0 != op_type.0 => {
                    compile_variable_read(emitter, ctx, variable, own)?;
                    crate::compile_arith::convert(emitter, own, op_type);
                    Ok(())
                }
                _ => compile_variable_read(emitter, ctx, variable, op_type),
            }
        }
        ExprKind::BinaryOp(binary) => {
            let result = expr_operand_name(ctx, expr);
            compile_binary_arith(emitter, ctx, binary, result.as_ref(), op_type)
        }
        ExprKind::UnaryOp(unary) => {
            let result = expr_operand_name(ctx, expr);
            compile_at_operand_type(
                emitter,
                ctx,
                result.as_ref(),
                op_type,
                |emitter, ctx, at| {
                    compile_expr(emitter, ctx, &unary.term, at)?;
                    match unary.op {
                        UnaryOp::Neg => {
                            emit_neg(emitter, at);
                            Ok(())
                        }
                        UnaryOp::Not => emit_not(emitter, ctx, at, &unary.term),
                    }
                },
            )
        }
        ExprKind::LateBound(late_bound) => {
            if let Some(ref_slot) = ctx.in_out_ref_slot(&late_bound.value) {
                emit_load_in_out(emitter, ref_slot);
                return Ok(());
            }
            let var_index = ctx.var_index(&late_bound.value)?;
            emit_load_var(emitter, var_index, op_type);
            Ok(())
        }
        ExprKind::Expression(inner) => compile_expr(emitter, ctx, inner, op_type),
        ExprKind::Compare(compare) => compile_compare(emitter, ctx, compare, op_type),
        ExprKind::EnumeratedValue(enum_val) => {
            // REQ-EN-codegen-030: Push the enum value's ordinal as an i32
            // constant, looked up in the type the analyzer gave the value.
            let members = crate::compile_enum::members_of_expr(ctx, expr);
            let ordinal = crate::compile_enum::ordinal_in(members, enum_val)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            Ok(())
        }
        ExprKind::Function(func) => {
            let result = expr_operand_name(ctx, expr);
            compile_function_call(emitter, ctx, func, result.as_ref(), op_type)
        }
        ExprKind::MethodCall(call) => compile_method_call_expression(emitter, ctx, call),
        ExprKind::Ref(variable) => {
            // REF(param) of a VAR_IN_OUT parameter is the reference its slot
            // already holds: the caller's variable.
            if let Some(ref_slot) = in_out_ref_slot(ctx, variable) {
                emitter.emit_load_var_i64(ref_slot);
                return Ok(());
            }
            // REF(var) → push the variable's table index as a u64 constant.
            let var_index = resolve_variable(ctx, variable)?;
            let pool_index = ctx.add_i64_constant(var_index.into());
            emitter.emit_load_const_i64(pool_index);
            Ok(())
        }
        ExprKind::Deref(inner) => {
            // var^ → compile the reference expression (produces a var index),
            // then emit LOAD_INDIRECT to load the referenced variable's value.
            compile_expr(emitter, ctx, inner, (OpWidth::W64, Signedness::Unsigned))?;
            emitter.emit_load_indirect();
            Ok(())
        }
        // The analyzer decided the conversion (ADR-0056): the inner value is
        // compiled at its own type, so it widens by its own signedness, and
        // converted to the type the node records.
        ExprKind::ImplicitConversion(inner) => {
            let from = self::op_type(ctx, inner)?;
            let to = self::op_type(ctx, expr)?;
            compile_expr(emitter, ctx, inner, from)?;
            crate::compile_arith::convert(emitter, from, to);
            crate::compile_arith::convert(emitter, to, op_type);
            Ok(())
        }
        ExprKind::Null(_) => {
            // NULL → push null sentinel (u64::MAX) as a u64 constant.
            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
            emitter.emit_load_const_i64(pool_index);
            Ok(())
        }
    }
}

/// Compiles a comparison, logical, or bitwise binary expression, leaving the
/// result on the stack.
///
/// `op_type` is the type context the enclosing expression supplies; it is only
/// a fallback, because a comparison's own result is BOOL while its operands may
/// be any type.
fn compile_compare(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    compare: &CompareExpr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    // AND_THEN and OR_ELSE must not evaluate their right operand when the
    // left one already decides the answer, so they branch instead of
    // evaluating both operands into an eager bitwise op.
    if let Some(short_circuit) = ShortCircuitOp::for_expr(ctx, compare) {
        return compile_short_circuit(emitter, ctx, compare, short_circuit);
    }

    if compare.op.is_comparison() {
        return compile_comparison(
            emitter,
            ctx,
            &compare.op,
            &compare.left,
            &compare.right,
            op_type,
        );
    }

    // AND, OR and XOR are boolean on BOOL operands and bitwise on a bit
    // string. Their result has the operand type, derived from a concrete
    // (non-generic) resolved type, preferring the left operand: when one
    // side is a literal (generic type like ANY_INT) and the other is a typed
    // variable (e.g. DWORD), the concrete type gives the right width.
    let operand_op_type = concrete_op_type_from_expr(ctx, &compare.left)
        .or_else(|| concrete_op_type_from_expr(ctx, &compare.right))
        .or_else(|| op_type_from_expr(ctx, &compare.left))
        .unwrap_or(op_type);
    compile_expr(emitter, ctx, &compare.left, operand_op_type)?;
    compile_expr(emitter, ctx, &compare.right, operand_op_type)?;
    emit_compare_op(emitter, &compare.op, operand_op_type);
    Ok(())
}

/// Compiles the integer count a time-like literal stores, pushing it onto the
/// stack: milliseconds for a duration or a time of day, seconds since
/// 1970-01-01 for a date or a date-and-time.
///
/// The count is held to the range the operation type names, which is the
/// storage it is about to go into: signed for a duration (ADR-0021 -- a
/// duration can be negative), unsigned for the calendar types (ADR-0025), at
/// whichever of the two widths the type uses. A count outside it is reported
/// as `problem` rather than truncated, because truncating leaves a value that
/// is not the one the program wrote -- `T#30d` kept its low 32 bits and became
/// -1,702,967,296 ms, a *negative* 19.7 days, with nothing said about it.
///
/// `count` is an `i128` so that the check happens before any narrowing: every
/// storage this can target, up to `u64::MAX`, and every value a literal can
/// carry, up to a duration's own `i128` millisecond count, fit it.
///
/// Every one of these is a count rather than a measurement, so no
/// floating-point type holds one and the analyzer rejects the assignment that
/// would ask for it (P4035). Reaching a float width here is a broken
/// invariant rather than a missing capability, and saying so beats what the
/// catch-all these arms shared used to do: emit an integer load, leaving the
/// count's bit pattern in a float slot to be read back as a number unrelated
/// to the literal.
fn compile_time_count(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    count: i128,
    literal: &str,
    span: &SourceSpan,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    // Each arm states the range it stores, so a width added to `OpWidth`
    // has to say what a count means at that width rather than inheriting an
    // answer from a catch-all.
    match op_type.0 {
        OpWidth::W32 => {
            let value = within_storage(count, 32, op_type.1, literal, span)?;
            let pool_index = ctx.add_i32_constant(value as i32);
            emitter.emit_load_const_i32(pool_index);
        }
        OpWidth::W64 => {
            let value = within_storage(count, 64, op_type.1, literal, span)?;
            let pool_index = ctx.add_i64_constant(value as i64);
            emitter.emit_load_const_i64(pool_index);
        }
        // A count is not a measurement, so no floating-point type holds one,
        // and the analyzer rejects the assignment that would ask for it
        // (P4035). Reaching here means analysis was skipped, which is a
        // broken invariant rather than a missing capability: the catch-all
        // this replaced emitted an integer load, leaving the count's bit
        // pattern in a float slot to be read back as a number unrelated to
        // the literal.
        OpWidth::F32 | OpWidth::F64 => {
            return Err(Diagnostic::internal_error_at(Label::span(
                span.clone(),
                format!("{literal} compiled at a floating-point operation width"),
            )))
        }
    }
    Ok(())
}

/// Returns `count`, or an internal error when a `bits`-wide integer of
/// `signedness` cannot hold it.
///
/// `rule_temporal_literal_range` holds every temporal literal to the range its
/// own type gives it, and a literal only reaches storage at least that wide --
/// narrowing one into the shorter type is rejected too. So by the time a count
/// arrives here it fits, and a count that does not is a broken invariant
/// rather than something to report against the program: the diagnostic names
/// the compiler, not the source.
///
/// The check remains because being wrong here is silent. Truncating a count
/// emits a different value than the program wrote -- `T#30d` became a
/// *negative* 19.7 days -- which no test of the program's behaviour would
/// attribute to codegen.
fn within_storage(
    count: i128,
    bits: u32,
    signedness: Signedness,
    literal: &str,
    span: &SourceSpan,
) -> Result<i128, Diagnostic> {
    let signed = signedness == Signedness::Signed;
    if !ironplc_analyzer::value_range::fits(count, bits, signed) {
        let (minimum, maximum) = ironplc_analyzer::value_range::for_integer(bits, signed);
        return Err(Diagnostic::internal_error_at(Label::span(
            span.clone(),
            format!(
                "{literal} holds {count}, outside the range {minimum} to {maximum} its type stores"
            ),
        )));
    }
    Ok(count)
}

/// Compiles a constant literal, pushing it onto the stack.
pub(crate) fn compile_constant(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    constant: &ConstantKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match constant {
        ConstantKind::IntegerLiteral(lit) => {
            let span = lit.value.value.span();
            match op_type {
                (OpWidth::W32, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        let unsigned = lit.value.value.value as i128;
                        let signed = -unsigned;
                        i32::try_from(signed).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &signed.to_string())
                        })?
                    } else {
                        i32::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })?
                    };
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W32, Signedness::Unsigned) => {
                    // Unsigned 32-bit: values up to u32::MAX are valid.
                    // Store the bit-pattern as i32.
                    let value = if lit.value.is_neg {
                        return Err(Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Integer literal"),
                        )
                        .with_context("value", &format!("-{}", lit.value.value.value)));
                    } else {
                        u32::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })? as i32
                    };
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W64, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        let unsigned = lit.value.value.value as i128;
                        let signed = -unsigned;
                        i64::try_from(signed).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &signed.to_string())
                        })?
                    } else {
                        i64::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })?
                    };
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
                (OpWidth::W64, Signedness::Unsigned) => {
                    // Unsigned 64-bit: values up to u64::MAX are valid.
                    // Store the bit-pattern as i64.
                    let value = if lit.value.is_neg {
                        return Err(Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Integer literal"),
                        )
                        .with_context("value", &format!("-{}", lit.value.value.value)));
                    } else {
                        lit.value.value.value as i64
                    };
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
                (OpWidth::F32, _) => {
                    // Integer literal in float context: convert to f32.
                    let int_val = if lit.value.is_neg {
                        -(lit.value.value.value as f32)
                    } else {
                        lit.value.value.value as f32
                    };
                    let pool_index = ctx.add_f32_constant(int_val);
                    emitter.emit_load_const_f32(pool_index);
                }
                (OpWidth::F64, _) => {
                    // Integer literal in float context: convert to f64.
                    let int_val = if lit.value.is_neg {
                        -(lit.value.value.value as f64)
                    } else {
                        lit.value.value.value as f64
                    };
                    let pool_index = ctx.add_f64_constant(int_val);
                    emitter.emit_load_const_f64(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::RealLiteral(lit) => match op_type.0 {
            OpWidth::F32 => {
                let value = lit.value as f32;
                let pool_index = ctx.add_f32_constant(value);
                emitter.emit_load_const_f32(pool_index);
                Ok(())
            }
            OpWidth::F64 => {
                let pool_index = ctx.add_f64_constant(lit.value);
                emitter.emit_load_const_f64(pool_index);
                Ok(())
            }
            _ => Err(Diagnostic::not_implemented(Label::span(
                lit.span.clone(),
                "Real literal in a non-floating-point context",
            ))),
        },
        ConstantKind::Boolean(lit) => {
            match lit.value {
                Boolean::True => emitter.emit_load_true(),
                Boolean::False => emitter.emit_load_false(),
            }
            Ok(())
        }
        ConstantKind::CharacterString(lit) => {
            // Load the string literal into a temp buffer, leaving buf_idx on the stack.
            // The caller (e.g., string assignment path) will consume the buf_idx via
            // emit_str_store_var to copy the value into the target data region.
            let bytes = encode_string_literal(&lit.value, NARROW_CHAR_WIDTH);
            let pool_index = ctx.add_str_constant(bytes);
            emitter.emit_load_const_str(pool_index);
            Ok(())
        }
        ConstantKind::Duration(lit) => compile_time_count(
            emitter,
            ctx,
            lit.interval.whole_milliseconds(),
            "Duration literal",
            &lit.span,
            op_type,
        ),
        // A time of day cannot leave its range: `whole_milliseconds` is
        // bounded by 86,399,999 by construction, which every width holds. The
        // check below is therefore vacuous, and the code names the problem it
        // would be if the bound ever stopped holding.
        ConstantKind::TimeOfDay(lit) => compile_time_count(
            emitter,
            ctx,
            i128::from(lit.whole_milliseconds()),
            "Time-of-day literal",
            &lit.span,
            op_type,
        ),
        ConstantKind::Date(lit) => compile_time_count(
            emitter,
            ctx,
            i128::from(lit.seconds_since_epoch()),
            "Date literal",
            &lit.span,
            op_type,
        ),
        ConstantKind::DateAndTime(lit) => compile_time_count(
            emitter,
            ctx,
            i128::from(lit.seconds_since_epoch()),
            "Date literal",
            &lit.span,
            op_type,
        ),
        ConstantKind::BitStringLiteral(lit) => {
            let span = lit.value.span();
            match op_type {
                (OpWidth::W32, _) => {
                    let value = u32::try_from(lit.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Bit string literal"),
                        )
                        .with_context("value", &lit.value.value.to_string())
                    })? as i32;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W64, _) => {
                    let value = u64::try_from(lit.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Bit string literal"),
                        )
                        .with_context("value", &lit.value.value.to_string())
                    })? as i64;
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
                (OpWidth::F32, _) => {
                    let value = lit.value.value as f32;
                    let pool_index = ctx.add_f32_constant(value);
                    emitter.emit_load_const_f32(pool_index);
                }
                (OpWidth::F64, _) => {
                    let value = lit.value.value as f64;
                    let pool_index = ctx.add_f64_constant(value);
                    emitter.emit_load_const_f64(pool_index);
                }
            }
            Ok(())
        }
    }
}

/// Compiles a variable read expression.
///
/// For simple named variables, loads the variable value onto the stack.
/// A bit access (e.g., `a.0`) or partial access (e.g., `a.%B1`) leaves the
/// bits it selects, shifted down to bit 0.
pub(crate) fn compile_variable_read(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    variable: &Variable,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    if let Some(access) = PartialAccess::of(variable) {
        return compile_partial_access_read(emitter, ctx, &access);
    }
    match variable {
        // The guard excludes `s.arr[i].field`, whose record is an array
        // element rather than a fixed-offset struct field. That shape falls
        // through to the generic `resolve_access` dispatch below.
        Variable::Symbolic(SymbolicVariableKind::Structured(structured))
            if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) =>
        {
            // Function block instance field read (e.g. `timer.Q`). FB instances
            // live in `ctx.fb_instances` rather than `ctx.struct_vars`, and
            // their fields are stored in the data region addressed via
            // FB_LOAD_PARAM.
            if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
                if let Some(fb_info) = ctx.fb_instances.get(&named.name) {
                    let field_name = structured.field.to_string().to_lowercase();
                    let field_idx =
                        fb_info
                            .field_indices
                            .get(&field_name)
                            .copied()
                            .ok_or_else(|| {
                                Diagnostic::not_implemented(Label::span(
                                    structured.field.span(),
                                    format!(
                                        "Unknown field '{}' on function block '{}' \
                                         (reading a PROPERTY is not supported yet)",
                                        structured.field, named.name
                                    ),
                                ))
                            })?;
                    let var_index = fb_info.var_index;
                    emitter.emit_fb_load_instance(var_index);
                    emitter.emit_fb_load_param(field_idx);
                    emitter.emit_swap();
                    emitter.emit_pop();
                    return Ok(());
                }
            }

            // STRING fields are composite (multi-slot) and stored in the data
            // region, so we intercept before resolve_struct_field_access which
            // only supports single-slot (primitive/enum) fields.
            let (root_name, slot_offset, field_type) = crate::compile_struct::walk_struct_chain(
                ctx,
                &structured.record,
                &structured.field,
                0,
            )?;
            if matches!(
                &field_type,
                ironplc_analyzer::semantic_type::SemanticType::String { .. }
            ) {
                // `walk_struct_chain` found this structure variable above.
                let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
                    Diagnostic::internal_error_at(Label::span(
                        structured.span(),
                        format!("Variable '{}' is not a structure", root_name),
                    ))
                })?;
                let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
                emitter.emit_str_load_var(byte_offset);
                return Ok(());
            }

            let (var_index, desc_index, slot_offset, _op_type, _field_type) =
                crate::compile_struct::resolve_struct_field_access(ctx, structured)?;
            let idx_const = ctx.add_i32_constant(slot_offset.raw() as i32);
            emitter.emit_load_const_i32(idx_const);
            emitter.emit_load_array(var_index, desc_index);
            Ok(())
        }
        _ => {
            // Check if this is a string variable (stored in data region).
            // String reads emit str_load_var to produce a buf_idx on the stack,
            // which is consumed by string assignment or string function args.
            if let Some(var_name) = resolve_variable_name(variable) {
                if let Some(info) = ctx.string_vars.get(var_name) {
                    let data_offset = info.data_offset;
                    emitter.emit_str_load_var(data_offset);
                    return Ok(());
                }
            }

            match crate::compile_array::resolve_access(ctx, variable)? {
                crate::compile_array::ResolvedAccess::Scalar { var_index } => {
                    emit_load_var(emitter, var_index, op_type);
                }
                crate::compile_array::ResolvedAccess::InOut { ref_slot } => {
                    emit_load_in_out(emitter, ref_slot);
                }
                crate::compile_array::ResolvedAccess::ArrayElement { info, subscripts } => {
                    let arr_var_index = info.var_index;
                    let arr_desc_index = info.desc_index;
                    let is_string_elem = info.is_string_element;
                    let dim_info: Vec<_> = info
                        .dimensions
                        .iter()
                        .map(|d| crate::compile_array::DimensionInfo {
                            lower_bound: d.lower_bound,
                            size: d.size,
                            stride: d.stride,
                        })
                        .collect();
                    let span = variable_span(variable);
                    crate::compile_array::emit_flat_index(
                        emitter,
                        ctx,
                        &subscripts,
                        &dim_info,
                        &span,
                    )?;
                    if is_string_elem {
                        emitter.emit_str_load_array_elem(arr_var_index, arr_desc_index);
                    } else {
                        emitter.emit_load_array(arr_var_index, arr_desc_index);
                    }
                }
                crate::compile_array::ResolvedAccess::DerefArrayElement { info, subscripts } => {
                    let ref_var_index = info.var_index;
                    let arr_desc_index = info.desc_index;
                    let dim_info: Vec<_> = info
                        .dimensions
                        .iter()
                        .map(|d| crate::compile_array::DimensionInfo {
                            lower_bound: d.lower_bound,
                            size: d.size,
                            stride: d.stride,
                        })
                        .collect();
                    let span = variable_span(variable);
                    crate::compile_array::emit_flat_index(
                        emitter,
                        ctx,
                        &subscripts,
                        &dim_info,
                        &span,
                    )?;
                    emitter.emit_load_array_deref(ref_var_index, arr_desc_index);
                }
                crate::compile_array::ResolvedAccess::StructFieldArrayElement {
                    var_index,
                    desc_index,
                    field_slot_offset,
                    ref dimensions,
                    subscripts,
                    ..
                } => {
                    let span = variable_span(variable);
                    crate::compile_array::emit_flat_index(
                        emitter,
                        ctx,
                        &subscripts,
                        dimensions,
                        &span,
                    )?;
                    let offset_const = ctx.add_i64_constant(field_slot_offset.raw() as i64);
                    emitter.emit_load_const_i64(offset_const);
                    emitter.emit_add_i64();
                    emitter.emit_load_array(var_index, desc_index);
                }
                crate::compile_array::ResolvedAccess::StructFieldStringArrayElement(element) => {
                    element.emit_base_and_index(emitter, ctx, &variable_span(variable))?;
                    emitter.emit_str_load_array_elem(
                        element.scratch_var_index,
                        element.string_desc_index,
                    );
                }
            }
            Ok(())
        }
    }
}

/// Returns the slot holding the reference when `variable` names a
/// `VAR_IN_OUT` parameter of the function being compiled.
pub(crate) fn in_out_ref_slot(ctx: &CompileContext, variable: &Variable) -> Option<VarIndex> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Named(named)) => ctx.in_out_ref_slot(&named.name),
        _ => None,
    }
}

/// Loads the value of the variable a `VAR_IN_OUT` parameter refers to.
pub(crate) fn emit_load_in_out(emitter: &mut Emitter, ref_slot: VarIndex) {
    emitter.emit_load_var_i64(ref_slot);
    emitter.emit_load_indirect();
}

/// Resolves a variable reference to its variable table index.
pub(crate) fn resolve_variable(
    ctx: &CompileContext,
    variable: &Variable,
) -> Result<VarIndex, Diagnostic> {
    match variable {
        Variable::Symbolic(symbolic) => match symbolic {
            SymbolicVariableKind::Named(named) => ctx.var_index(&named.name),
            SymbolicVariableKind::Array(array) => Err(Diagnostic::todo_with_span(array.span())),
            SymbolicVariableKind::Structured(structured) => {
                Err(Diagnostic::todo_with_span(structured.span()))
            }
            SymbolicVariableKind::BitAccess(bit_access) => {
                Err(Diagnostic::todo_with_span(bit_access.span()))
            }
            SymbolicVariableKind::PartialAccess(pa) => Err(Diagnostic::todo_with_span(pa.span())),
            SymbolicVariableKind::Deref(deref) => Err(Diagnostic::todo_with_span(deref.span())),
            SymbolicVariableKind::SelfRef(self_ref) => {
                Err(Diagnostic::todo_with_span(self_ref.span()))
            }
        },
        Variable::Direct(direct) => Err(Diagnostic::todo_with_span(direct.position.clone())),
    }
}

/// Extracts the variable name `Id` from a variable reference, if it is a named symbolic variable.
pub(crate) fn resolve_variable_name(variable: &Variable) -> Option<&Id> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Named(named)) => Some(&named.name),
        _ => None,
    }
}

/// Extracts a SourceSpan from a Variable for diagnostic messages.
pub(crate) fn variable_span(variable: &Variable) -> ironplc_dsl::core::SourceSpan {
    match variable {
        Variable::Symbolic(kind) => kind.span(),
        Variable::Direct(addr) => addr.position.clone(),
    }
}

/// Result of classifying a comparison expression as a fusable
/// `var <cmp> const` shape eligible for emission via `CMP_BR_*`.
#[derive(Clone, Copy)]
pub(crate) struct ClassifiedCmp {
    /// `cmp_op` byte (one of `opcode::cmp_op::*`), already adjusted so the
    /// variable is on the LHS and the constant on the RHS.
    pub cmp_op_byte: u8,
    /// Variable table index of the LHS.
    pub var_index: VarIndex,
    /// Constant pool index of the RHS literal.
    pub const_idx: u16,
    /// Operand op width: `W32` (→ `CMP_BR_I32`) or `W64` (→ `CMP_BR_I64`).
    pub op_width: OpWidth,
}

/// Recognises the fusable shape `var <cmp> const_literal` (or
/// `const_literal <cmp> var`, which is commuted), where the variable is a
/// simple named scalar of a 32- or 64-bit signed integer type and the
/// constant is an integer literal that fits the variable's width.
///
/// Returns `Some` with the constant pooled and the comparison operator
/// resolved to a `cmp_op` byte. Returns `None` for any unsupported shape
/// (float compares, unsigned compares, var-var, complex LHS/RHS, etc.) so
/// the caller can fall back to the unfused emission.
pub(crate) fn try_classify_cmp(ctx: &mut CompileContext, expr: &Expr) -> Option<ClassifiedCmp> {
    let compare = match peel(expr) {
        ExprKind::Compare(c) => c,
        _ => return None,
    };
    let cmp_op_byte = compare_op_to_cmp_op(&compare.op)?;

    let left_kind = peel(&compare.left);
    let right_kind = peel(&compare.right);

    // Try `var <cmp> const`.
    if let (Some(name), Some(value)) = (named_variable_name(left_kind), constant_i64(right_kind)) {
        return classify_with_named(ctx, name, value, cmp_op_byte);
    }
    // Try `const <cmp> var` — commute to `var <cmp> const`.
    if let (Some(value), Some(name)) = (constant_i64(left_kind), named_variable_name(right_kind)) {
        let commuted = opcode::cmp_op::commute(cmp_op_byte)?;
        return classify_with_named(ctx, name, value, commuted);
    }
    None
}

fn classify_with_named(
    ctx: &mut CompileContext,
    name: &Id,
    value: i64,
    cmp_op_byte: u8,
) -> Option<ClassifiedCmp> {
    let info = ctx.var_type_info(name)?;
    if info.signedness != Signedness::Signed {
        return None;
    }
    let var_index = ctx.var_index(name).ok()?;
    match info.op_width {
        OpWidth::W32 => {
            let v32 = i32::try_from(value).ok()?;
            let const_idx = ctx.add_i32_constant(v32);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W32,
            })
        }
        OpWidth::W64 => {
            let const_idx = ctx.add_i64_constant(value);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W64,
            })
        }
        OpWidth::F32 | OpWidth::F64 => None,
    }
}

/// Emits a `CMP_BR_*` instruction for a previously classified comparison,
/// branching to `target` when the (possibly negated) predicate evaluates
/// to true. When `branch_when_true` is `false`, the comparison operator
/// is negated so the branch fires on the false-polarity (e.g. for
/// "branch to END if NOT cond" zero-trip and IF skip patterns).
pub(crate) fn emit_classified_cmp_br(
    emitter: &mut crate::emit::Emitter,
    classified: ClassifiedCmp,
    branch_when_true: bool,
    target: crate::emit::Label,
) -> Result<(), Diagnostic> {
    let cmp_op_byte = if branch_when_true {
        classified.cmp_op_byte
    } else {
        opcode::cmp_op::negate(classified.cmp_op_byte).ok_or_else(Diagnostic::internal_error)?
    };
    match classified.op_width {
        OpWidth::W32 => emitter.emit_cmp_br_i32(
            cmp_op_byte,
            classified.var_index,
            classified.const_idx,
            target,
        ),
        OpWidth::W64 => emitter.emit_cmp_br_i64(
            cmp_op_byte,
            classified.var_index,
            classified.const_idx,
            target,
        ),
        OpWidth::F32 | OpWidth::F64 => return Err(Diagnostic::internal_error()),
    }
    Ok(())
}

/// Strips parenthesised-expression wrappers from an `ExprKind`.
fn peel(expr: &Expr) -> &ExprKind {
    let mut current = &expr.kind;
    while let ExprKind::Expression(inner) = current {
        current = &inner.kind;
    }
    current
}

/// Returns the identifier of a simple named scalar variable reference,
/// or `None` for any other variable shape (array element, struct field,
/// bit access, dereference, etc.).
fn named_variable_name(kind: &ExprKind) -> Option<&Id> {
    match kind {
        ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
            Some(&named.name)
        }
        ExprKind::LateBound(late) => Some(&late.value),
        _ => None,
    }
}

/// Returns the `i64` value of an `ExprKind` that is a compile-time integer
/// literal (positive, negative, or unary-negated). Returns `None` for any
/// other expression, and for a literal outside the `i64` range.
pub(crate) fn constant_i64(kind: &ExprKind) -> Option<i64> {
    match kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => signed_integer_to_i64(&lit.value),
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => match &unary.term.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
                signed_integer_to_i64(&lit.value).and_then(i64::checked_neg)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Maps a `CompareOp` AST node to the `opcode::cmp_op` byte used by
/// `CMP_BR_*`. Returns `None` for non-comparison (logical/bitwise)
/// operators, which `CMP_BR_*` does not support.
fn compare_op_to_cmp_op(op: &CompareOp) -> Option<u8> {
    match op {
        CompareOp::Eq => Some(opcode::cmp_op::EQ),
        CompareOp::Ne => Some(opcode::cmp_op::NE),
        CompareOp::Lt => Some(opcode::cmp_op::LT_S),
        CompareOp::LtEq => Some(opcode::cmp_op::LE_S),
        CompareOp::Gt => Some(opcode::cmp_op::GT_S),
        CompareOp::GtEq => Some(opcode::cmp_op::GE_S),
        CompareOp::And
        | CompareOp::Or
        | CompareOp::Xor
        | CompareOp::AndThen
        | CompareOp::OrElse => None,
    }
}

/// Converts a `SignedInteger` AST node to an `i64` value, or `None` when it
/// is outside the `i64` range.
///
/// A caller is looking for a constant to fuse into a comparison or to bound a
/// `FOR` loop with, and compiles the expression the ordinary way when there is
/// none, so a literal that does not fit is not a problem to report here.
fn signed_integer_to_i64(si: &SignedInteger) -> Option<i64> {
    if si.is_neg {
        i64::try_from(-(si.value.value as i128)).ok()
    } else {
        i64::try_from(si.value.value).ok()
    }
}

// --- Typed opcode emission helpers ---
//
// Each helper selects the correct opcode based on the operation type
// (width and/or signedness).

pub(crate) fn emit_truncation(emitter: &mut Emitter, type_info: VarTypeInfo) {
    match (
        type_info.op_width,
        type_info.signedness,
        type_info.storage_bits,
    ) {
        (OpWidth::W32, Signedness::Signed, 8) => emitter.emit_trunc_i8(),
        (OpWidth::W32, Signedness::Signed, 16) => emitter.emit_trunc_i16(),
        (OpWidth::W32, Signedness::Unsigned, 8) => emitter.emit_trunc_u8(),
        (OpWidth::W32, Signedness::Unsigned, 16) => emitter.emit_trunc_u16(),
        // 32-bit and 64-bit types fill their native width; no truncation needed.
        _ => {}
    }
}

pub(crate) fn emit_load_var(emitter: &mut Emitter, var_index: VarIndex, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_load_var_i32(var_index),
        OpWidth::W64 => emitter.emit_load_var_i64(var_index),
        OpWidth::F32 => emitter.emit_load_var_f32(var_index),
        OpWidth::F64 => emitter.emit_load_var_f64(var_index),
    }
}

pub(crate) fn emit_store_var(emitter: &mut Emitter, var_index: VarIndex, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_store_var_i32(var_index),
        OpWidth::W64 => emitter.emit_store_var_i64(var_index),
        OpWidth::F32 => emitter.emit_store_var_f32(var_index),
        OpWidth::F64 => emitter.emit_store_var_f64(var_index),
    }
}

// The `emit_<op>` dispatch functions below map an operand `OpType` to the
// matching typed `Emitter` method. They share a handful of identical shapes,
// so the three `macro_rules!` generators fold each shape into a single
// definition. Every generated function keeps the same `pub(crate)` name and
// `(emitter, op_type)` signature that existing call sites and function-pointer
// uses in `compile_call.rs` / `compile_stmt.rs` rely on.

/// Generates a width-only dispatch `emit_<stem>` mapping the operand width to
/// `emit_<stem>_{i32,i64,f32,f64}`. Used for operators whose signedness does
/// not change the opcode.
macro_rules! emit_width_op {
    ($stem:ident) => {
        paste! {
            pub(crate) fn [<emit_ $stem>](emitter: &mut Emitter, op_type: OpType) {
                match op_type.0 {
                    OpWidth::W32 => emitter.[<emit_ $stem _i32>](),
                    OpWidth::W64 => emitter.[<emit_ $stem _i64>](),
                    OpWidth::F32 => emitter.[<emit_ $stem _f32>](),
                    OpWidth::F64 => emitter.[<emit_ $stem _f64>](),
                }
            }
        }
    };
}

/// Generates a width+signedness dispatch `emit_<stem>` mapping
/// `(OpWidth, Signedness)` to `emit_<stem>_{i32,u32,i64,u64,f32,f64}`. Integer
/// widths pick signed/unsigned variants; float widths ignore signedness.
macro_rules! emit_signed_op {
    ($stem:ident) => {
        paste! {
            pub(crate) fn [<emit_ $stem>](emitter: &mut Emitter, op_type: OpType) {
                match op_type {
                    (OpWidth::W32, Signedness::Signed) => emitter.[<emit_ $stem _i32>](),
                    (OpWidth::W32, Signedness::Unsigned) => emitter.[<emit_ $stem _u32>](),
                    (OpWidth::W64, Signedness::Signed) => emitter.[<emit_ $stem _i64>](),
                    (OpWidth::W64, Signedness::Unsigned) => emitter.[<emit_ $stem _u64>](),
                    (OpWidth::F32, _) => emitter.[<emit_ $stem _f32>](),
                    (OpWidth::F64, _) => emitter.[<emit_ $stem _f64>](),
                }
            }
        }
    };
}

/// Generates a logical/bitwise dispatch `emit_<stem>` for AND/OR/XOR. Unsigned
/// integer operands emit the bitwise opcode (`emit_bit_<stem>_32/_64`); every
/// other type (BOOL and signed integers) emits `emit_bool_<stem>`.
macro_rules! emit_logical_op {
    ($stem:ident) => {
        paste! {
            pub(crate) fn [<emit_ $stem>](emitter: &mut Emitter, op_type: OpType) {
                match op_type {
                    (OpWidth::W32, Signedness::Unsigned) => emitter.[<emit_bit_ $stem _32>](),
                    (OpWidth::W64, Signedness::Unsigned) => emitter.[<emit_bit_ $stem _64>](),
                    _ => emitter.[<emit_bool_ $stem>](),
                }
            }
        }
    };
}

emit_width_op!(add);
emit_width_op!(sub);
emit_width_op!(mul);
emit_width_op!(neg);
emit_width_op!(eq);
emit_width_op!(ne);

emit_signed_op!(div);
emit_signed_op!(lt);
emit_signed_op!(le);
emit_signed_op!(gt);
emit_signed_op!(ge);

emit_logical_op!(and);
emit_logical_op!(or);
emit_logical_op!(xor);

/// Emits the opcode of a binary arithmetic operator for operands of `op_type`.
///
/// The operator expression and the function form of the operator (`ADD`,
/// `SUB`, ...) both come through here, so the two spellings cannot emit
/// differently.
pub(crate) fn emit_arithmetic_op(emitter: &mut Emitter, op: &Operator, op_type: OpType) {
    match op {
        Operator::Add => emit_add(emitter, op_type),
        Operator::Sub => emit_sub(emitter, op_type),
        Operator::Mul => emit_mul(emitter, op_type),
        Operator::Div => emit_div(emitter, op_type),
        Operator::Mod => emit_mod(emitter, op_type),
        Operator::Pow => emit_pow(emitter, op_type),
    }
}

/// Emits the opcode of a comparison, logical or bitwise operator for
/// operands of `op_type`.
///
/// The operator expression and the function form of the operator (`GT`,
/// `AND`, ...) both come through here, so the two spellings cannot emit
/// differently.
pub(crate) fn emit_compare_op(emitter: &mut Emitter, op: &CompareOp, op_type: OpType) {
    match op {
        CompareOp::Eq => emit_eq(emitter, op_type),
        CompareOp::Ne => emit_ne(emitter, op_type),
        CompareOp::Lt => emit_lt(emitter, op_type),
        CompareOp::Gt => emit_gt(emitter, op_type),
        CompareOp::LtEq => emit_le(emitter, op_type),
        CompareOp::GtEq => emit_ge(emitter, op_type),
        CompareOp::And => emit_and(emitter, op_type),
        CompareOp::Or => emit_or(emitter, op_type),
        CompareOp::Xor => emit_xor(emitter, op_type),
        // Only reached for non-BOOL operands, where there is no boolean to
        // short-circuit on and the operator degenerates to its eager
        // counterpart. See `ShortCircuitOp::for_expr`.
        CompareOp::AndThen => emit_and(emitter, op_type),
        CompareOp::OrElse => emit_or(emitter, op_type),
    }
}

/// Emits the complement of the value of `term`, already on the stack.
///
/// The unsigned operation types are the bit strings, so they take the
/// bitwise complement, truncated back to `term`'s storage width where the
/// 32-bit complement would widen a BYTE or WORD. Every other type takes the
/// boolean complement. The `NOT` operator and the `NOT` function form both
/// come through here, so the two spellings cannot emit differently.
pub(crate) fn emit_not(
    emitter: &mut Emitter,
    ctx: &CompileContext,
    op_type: OpType,
    term: &Expr,
) -> Result<(), Diagnostic> {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => {
            emitter.emit_bit_not_32();
            match storage_bits(ctx, term)? {
                8 => emitter.emit_trunc_u8(),
                16 => emitter.emit_trunc_u16(),
                _ => {}
            }
        }
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_not_64(),
        _ => emitter.emit_bool_not(),
    }
    Ok(())
}

// Hand-written one-offs that do not fit the generated shapes above:

/// MOD dispatch. Fits the width+signedness shape for integers, but IEC 61131-3
/// MOD is integer-only, so the float arms are a no-op rather than a call to a
/// (nonexistent) `emit_mod_f32`. The analyzer rejects a float MOD before
/// codegen: the function form through the `MOD` row of the operator-form
/// table (P4026), the operator through `rule_operator_operand_type_check`
/// (P4049).
pub(crate) fn emit_mod(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_mod_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_mod_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_mod_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_mod_u64(),
        // Unreachable from source: the analyzer has already rejected a float
        // MOD (see above). Emitting nothing leaves the operand stack
        // unbalanced, which the bytecode verifier reports as an internal
        // error, so a gap in the analyzer cannot produce a silent miscompile.
        (OpWidth::F32, _) | (OpWidth::F64, _) => {}
    }
}

/// POW dispatch. Width-only shape, but emits `EXPT_*` builtins rather than
/// dedicated `emit_pow_*` opcodes, so it stays hand-written.
pub(crate) fn emit_pow(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_builtin(opcode::builtin::EXPT_I32),
        OpWidth::W64 => emitter.emit_builtin(opcode::builtin::EXPT_I64),
        OpWidth::F32 => emitter.emit_builtin(opcode::builtin::EXPT_F32),
        OpWidth::F64 => emitter.emit_builtin(opcode::builtin::EXPT_F64),
    }
}
