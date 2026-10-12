//! Emits a lowered expression.

use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    BinaryOp, CompareOp, Const, Expr, ExprKind, NarrowWidth, ShortCircuitOp, UnaryOp,
};
use ironplc_lowering::Lowering;

use super::{named, op_type, storage};
use crate::compile::{CompileContext, OpWidth, Signedness};
use crate::compile_arith::convert;
use crate::compile_expr::{
    emit_add, emit_div, emit_eq, emit_ge, emit_gt, emit_le, emit_lt, emit_mod, emit_mul, emit_ne,
    emit_neg, emit_sub,
};
use crate::compile_place::Place;
use crate::emit::Emitter;

pub(super) fn emit_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    expr: &Expr,
) -> Result<(), Diagnostic> {
    let op = op_type(expr.ty()).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            expr.span().clone(),
            "A lowered expression has no operation type",
        ))
    })?;
    match expr.kind() {
        ExprKind::Const(constant) => emit_constant(emitter, ctx, *constant),
        ExprKind::Read(place) => {
            let storage = named(storage(lowering, place, expr.span())?);
            Place::resolve(ctx, &storage)?.emit_load(emitter, ctx)?;
        }
        ExprKind::Convert(value) => {
            emit_expr(emitter, ctx, lowering, value)?;
            let from = op_type(value.ty()).unwrap_or(op);
            convert(emitter, from, op)?;
        }
        ExprKind::Truncate { width, value } => {
            emit_expr(emitter, ctx, lowering, value)?;
            emit_truncate(emitter, *width, op.1, expr.span())?;
        }
        ExprKind::Unary { op: unary, value } => {
            emit_expr(emitter, ctx, lowering, value)?;
            match unary {
                UnaryOp::Neg => emit_neg(emitter, op),
                UnaryOp::BitNot => match op.0 {
                    OpWidth::W64 => emitter.emit_bit_not_64(),
                    OpWidth::W32 | OpWidth::F32 | OpWidth::F64 => emitter.emit_bit_not_32(),
                },
                UnaryOp::BoolNot => emitter.emit_bool_not(),
            }
        }
        ExprKind::Binary {
            op: binary,
            lhs,
            rhs,
        } => {
            emit_expr(emitter, ctx, lowering, lhs)?;
            emit_expr(emitter, ctx, lowering, rhs)?;
            let wide = op.0 == OpWidth::W64;
            match binary {
                BinaryOp::Add => emit_add(emitter, op),
                BinaryOp::Sub => emit_sub(emitter, op),
                BinaryOp::Mul => emit_mul(emitter, op),
                BinaryOp::Div => emit_div(emitter, op),
                BinaryOp::Mod => emit_mod(emitter, op),
                BinaryOp::BitAnd if wide => emitter.emit_bit_and_64(),
                BinaryOp::BitAnd => emitter.emit_bit_and_32(),
                BinaryOp::BitOr if wide => emitter.emit_bit_or_64(),
                BinaryOp::BitOr => emitter.emit_bit_or_32(),
                BinaryOp::BitXor if wide => emitter.emit_bit_xor_64(),
                BinaryOp::BitXor => emitter.emit_bit_xor_32(),
                BinaryOp::BoolAnd => emitter.emit_bool_and(),
                BinaryOp::BoolOr => emitter.emit_bool_or(),
                BinaryOp::BoolXor => emitter.emit_bool_xor(),
            }
        }
        ExprKind::Compare {
            op: compare,
            lhs,
            rhs,
        } => {
            emit_expr(emitter, ctx, lowering, lhs)?;
            emit_expr(emitter, ctx, lowering, rhs)?;
            let operands = op_type(lhs.ty()).unwrap_or(op);
            match compare {
                CompareOp::Eq => emit_eq(emitter, operands),
                CompareOp::Ne => emit_ne(emitter, operands),
                CompareOp::Lt => emit_lt(emitter, operands),
                CompareOp::Le => emit_le(emitter, operands),
                CompareOp::Gt => emit_gt(emitter, operands),
                CompareOp::Ge => emit_ge(emitter, operands),
            }
        }
        ExprKind::ShortCircuit {
            op: short_circuit,
            lhs,
            rhs,
        } => {
            // As `compile_short_circuit` emits it: the right operand runs only
            // when the left one does not decide the result.
            let alternative = emitter.create_label();
            let end = emitter.create_label();
            emit_expr(emitter, ctx, lowering, lhs)?;
            emitter.emit_jmp_if_not(alternative);
            let depth_before_arms = emitter.stack_depth();
            match short_circuit {
                ShortCircuitOp::AndThen => emit_expr(emitter, ctx, lowering, rhs)?,
                ShortCircuitOp::OrElse => emitter.emit_load_true(),
            }
            emitter.emit_jmp(end);
            emitter.bind_label(alternative);
            emitter.reset_stack_depth(depth_before_arms);
            match short_circuit {
                ShortCircuitOp::AndThen => emitter.emit_load_false(),
                ShortCircuitOp::OrElse => emit_expr(emitter, ctx, lowering, rhs)?,
            }
            emitter.bind_label(end);
        }
    }
    Ok(())
}

/// Narrows the value on the stack to `width` bits of `signedness`.
pub(super) fn emit_truncate(
    emitter: &mut Emitter,
    width: NarrowWidth,
    signedness: Signedness,
    span: &ironplc_dsl::core::SourceSpan,
) -> Result<(), Diagnostic> {
    match (width, signedness) {
        (NarrowWidth::B8, Signedness::Signed) => emitter.emit_trunc_i8(),
        (NarrowWidth::B8, Signedness::Unsigned) => emitter.emit_trunc_u8(),
        (NarrowWidth::B16, Signedness::Signed) => emitter.emit_trunc_i16(),
        (NarrowWidth::B16, Signedness::Unsigned) => emitter.emit_trunc_u16(),
        // The bytecode backend keeps no value narrower than its slot at
        // 32 bits, so `can_emit` leaves such a truncation to the AST.
        (NarrowWidth::B32, _) => {
            return Err(Diagnostic::internal_error_at(Label::span(
                span.clone(),
                "A truncation to 32 bits",
            )))
        }
    }
    Ok(())
}

pub(super) fn emit_constant(emitter: &mut Emitter, ctx: &mut CompileContext, constant: Const) {
    match constant {
        Const::Bool(true) => emitter.emit_load_true(),
        Const::Bool(false) => emitter.emit_load_false(),
        Const::I32(value) => {
            let index = ctx.add_i32_constant(value);
            emitter.emit_load_const_i32(index);
        }
        Const::I64(value) => {
            let index = ctx.add_i64_constant(value);
            emitter.emit_load_const_i64(index);
        }
        Const::F32(value) => {
            let index = ctx.add_f32_constant(value);
            emitter.emit_load_const_f32(index);
        }
        Const::F64(value) => {
            let index = ctx.add_f64_constant(value);
            emitter.emit_load_const_f64(index);
        }
    }
}
