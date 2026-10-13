//! Emits a lowered expression.

use ironplc_container::opcode::builtin;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    Arg, BinaryOp, Callee, CompareOp, Const, Expr, ExprKind, Intrinsic, NarrowWidth,
    ShortCircuitOp, UnaryOp,
};
use ironplc_lowering::Lowering;

use super::op_type;
use crate::compile::{CompileContext, OpWidth, Signedness};
use crate::compile_arith::convert;
use crate::compile_expr::{
    emit_add, emit_div, emit_eq, emit_ge, emit_gt, emit_le, emit_lt, emit_mod, emit_mul, emit_ne,
    emit_neg, emit_sub,
};
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
            super::place::resolve(lowering, ctx, place, expr.span())?.emit_load(emitter, ctx)?;
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
        ExprKind::Call {
            callee: Callee::Intrinsic(intrinsic),
            args,
        } => {
            // `can_emit` leaves to the AST a call with no builtin.
            let builtin = builtin_of(*intrinsic, args.len()).ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    expr.span().clone(),
                    "A call of an intrinsic with no builtin",
                ))
            })?;
            for Arg::Value(value) in args {
                emit_expr(emitter, ctx, lowering, value)?;
            }
            emitter.emit_builtin(builtin)?;
        }
    }
    Ok(())
}

/// The VM builtin that computes a call of `intrinsic` with `args` arguments
/// (ADR-0008); `None` for a `MUX` of more inputs than the VM has a builtin
/// for. The match names every intrinsic, so one the IR adds does not compile
/// until it has a builtin here (REQ-LOW-codegen-074).
pub(super) fn builtin_of(intrinsic: Intrinsic, args: usize) -> Option<u16> {
    // A `MUX` has a builtin for each number of inputs after its selector.
    let mux = |base: u16| {
        u16::try_from(args.saturating_sub(1))
            .ok()
            .filter(|inputs| *inputs <= builtin::MUX_MAX_INPUTS)
            .map(|inputs| base + inputs)
    };
    Some(match intrinsic {
        Intrinsic::ShlU32 => builtin::SHL_I32,
        Intrinsic::ShlU64 => builtin::SHL_I64,
        Intrinsic::ShrU32 => builtin::SHR_I32,
        Intrinsic::ShrU64 => builtin::SHR_I64,
        Intrinsic::RolU8 => builtin::ROL_U8,
        Intrinsic::RolU16 => builtin::ROL_U16,
        Intrinsic::RolU32 => builtin::ROL_I32,
        Intrinsic::RolU64 => builtin::ROL_I64,
        Intrinsic::RorU8 => builtin::ROR_U8,
        Intrinsic::RorU16 => builtin::ROR_U16,
        Intrinsic::RorU32 => builtin::ROR_I32,
        Intrinsic::RorU64 => builtin::ROR_I64,
        Intrinsic::AbsI32 => builtin::ABS_I32,
        Intrinsic::AbsI64 => builtin::ABS_I64,
        Intrinsic::AbsF32 => builtin::ABS_F32,
        Intrinsic::AbsF64 => builtin::ABS_F64,
        Intrinsic::ExptI32 => builtin::EXPT_I32,
        Intrinsic::ExptI64 => builtin::EXPT_I64,
        Intrinsic::ExptF32 => builtin::EXPT_F32,
        Intrinsic::ExptF64 => builtin::EXPT_F64,
        Intrinsic::SqrtF32 => builtin::SQRT_F32,
        Intrinsic::SqrtF64 => builtin::SQRT_F64,
        Intrinsic::LnF32 => builtin::LN_F32,
        Intrinsic::LnF64 => builtin::LN_F64,
        Intrinsic::LogF32 => builtin::LOG_F32,
        Intrinsic::LogF64 => builtin::LOG_F64,
        Intrinsic::ExpF32 => builtin::EXP_F32,
        Intrinsic::ExpF64 => builtin::EXP_F64,
        Intrinsic::SinF32 => builtin::SIN_F32,
        Intrinsic::SinF64 => builtin::SIN_F64,
        Intrinsic::CosF32 => builtin::COS_F32,
        Intrinsic::CosF64 => builtin::COS_F64,
        Intrinsic::TanF32 => builtin::TAN_F32,
        Intrinsic::TanF64 => builtin::TAN_F64,
        Intrinsic::AsinF32 => builtin::ASIN_F32,
        Intrinsic::AsinF64 => builtin::ASIN_F64,
        Intrinsic::AcosF32 => builtin::ACOS_F32,
        Intrinsic::AcosF64 => builtin::ACOS_F64,
        Intrinsic::AtanF32 => builtin::ATAN_F32,
        Intrinsic::AtanF64 => builtin::ATAN_F64,
        Intrinsic::Atan2F32 => builtin::ATAN2_F32,
        Intrinsic::Atan2F64 => builtin::ATAN2_F64,
        Intrinsic::TruncF32 => builtin::TRUNC_F32,
        Intrinsic::TruncF64 => builtin::TRUNC_F64,
        Intrinsic::ModF32 => builtin::MOD_F32,
        Intrinsic::ModF64 => builtin::MOD_F64,
        Intrinsic::MinI32 => builtin::MIN_I32,
        Intrinsic::MinU32 => builtin::MIN_U32,
        Intrinsic::MinI64 => builtin::MIN_I64,
        Intrinsic::MinU64 => builtin::MIN_U64,
        Intrinsic::MinF32 => builtin::MIN_F32,
        Intrinsic::MinF64 => builtin::MIN_F64,
        Intrinsic::MaxI32 => builtin::MAX_I32,
        Intrinsic::MaxU32 => builtin::MAX_U32,
        Intrinsic::MaxI64 => builtin::MAX_I64,
        Intrinsic::MaxU64 => builtin::MAX_U64,
        Intrinsic::MaxF32 => builtin::MAX_F32,
        Intrinsic::MaxF64 => builtin::MAX_F64,
        Intrinsic::LimitI32 => builtin::LIMIT_I32,
        Intrinsic::LimitU32 => builtin::LIMIT_U32,
        Intrinsic::LimitI64 => builtin::LIMIT_I64,
        Intrinsic::LimitU64 => builtin::LIMIT_U64,
        Intrinsic::LimitF32 => builtin::LIMIT_F32,
        Intrinsic::LimitF64 => builtin::LIMIT_F64,
        Intrinsic::SelI32 | Intrinsic::SelU32 => builtin::SEL_I32,
        Intrinsic::SelI64 | Intrinsic::SelU64 => builtin::SEL_I64,
        Intrinsic::SelF32 => builtin::SEL_F32,
        Intrinsic::SelF64 => builtin::SEL_F64,
        // `MUX` only selects, as `SEL` does.
        Intrinsic::MuxI32 | Intrinsic::MuxU32 => return mux(builtin::MUX_I32_BASE),
        Intrinsic::MuxI64 | Intrinsic::MuxU64 => return mux(builtin::MUX_I64_BASE),
        Intrinsic::MuxF32 => return mux(builtin::MUX_F32_BASE),
        Intrinsic::MuxF64 => return mux(builtin::MUX_F64_BASE),
        Intrinsic::BcdToIntU8 => builtin::BCD_TO_INT_8,
        Intrinsic::BcdToIntU16 => builtin::BCD_TO_INT_16,
        Intrinsic::BcdToIntU32 => builtin::BCD_TO_INT_32,
        Intrinsic::BcdToIntU64 => builtin::BCD_TO_INT_64,
        // `INT_TO_BCD` reads its input's bits as unsigned whatever its sign.
        Intrinsic::IntToBcdI8 | Intrinsic::IntToBcdU8 => builtin::INT_TO_BCD_8,
        Intrinsic::IntToBcdI16 | Intrinsic::IntToBcdU16 => builtin::INT_TO_BCD_16,
        Intrinsic::IntToBcdI32 | Intrinsic::IntToBcdU32 => builtin::INT_TO_BCD_32,
        Intrinsic::IntToBcdI64 | Intrinsic::IntToBcdU64 => builtin::INT_TO_BCD_64,
    })
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
        (NarrowWidth::B32, Signedness::Signed | Signedness::Unsigned) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A `MUX` has a builtin for each number of inputs after its selector
    /// that the VM takes, and none for more.
    #[test]
    fn builtin_of_when_mux_then_builtin_for_its_number_of_inputs() {
        assert_eq!(
            builtin_of(Intrinsic::MuxF64, 3),
            Some(builtin::MUX_F64_BASE + 2)
        );
        assert_eq!(
            builtin_of(Intrinsic::MuxU32, 17),
            Some(builtin::MUX_I32_BASE + 16)
        );
        assert_eq!(builtin_of(Intrinsic::MuxI64, 18), None);
    }
}
