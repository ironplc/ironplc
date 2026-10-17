//! Emits a lowered expression.

use ironplc_container::opcode::builtin;
use ironplc_container::opcode::builtin::str_to_num::Target;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    Arg, BinaryOp, Callee, CompareOp, Const, Expr, ExprKind, Intrinsic, NarrowWidth, Place,
    ShortCircuitOp, UnaryOp,
};
use ironplc_lowering::Lowering;

use super::call_args::emit_passed;
use super::fn_call::take_result;
use super::method::{emit_method_call, method_site};
use super::place::{resolve_load, variable_name, Load};
use super::policy::parse_builtin;
use super::string::str_arg_offset;
use super::{op_type, user_function};
use crate::compile::{CompileContext, OpWidth, Signedness, UserFunctionInfo};
use crate::compile_arith::convert;
use crate::compile_expr::{
    emit_add, emit_div, emit_eq, emit_ge, emit_gt, emit_le, emit_lt, emit_mod, emit_mul, emit_ne,
    emit_neg, emit_null, emit_sub,
};
use crate::compile_subscript::emit_subscript_check;
use crate::compile_user_call::emit_reference;
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
        // The result of a call with outputs, which the call left on the
        // stack (`fn_call.rs`).
        ExprKind::Read(place) if take_result(ctx, place) => {}
        ExprKind::Read(place) => match resolve_load(lowering, ctx, place, expr.span())? {
            Load::Stored(storage) => storage.emit_load(emitter, ctx)?,
            Load::Referenced(storage) => {
                storage.emit_load(emitter, ctx)?;
                emitter.emit_load_indirect();
            }
        },
        ExprKind::RefTo(place) => emit_ref(emitter, ctx, lowering, place, expr.span())?,
        ExprKind::Null => emit_null(emitter, ctx),
        ExprKind::Convert(value)
        | ExprKind::ConvertChecked(value)
        | ExprKind::Truncate { value, .. }
        | ExprKind::Unary { value, .. } => {
            emit_expr(emitter, ctx, lowering, value)?;
            emit_operation(emitter, expr)?;
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
            callee: Callee::User(pou),
            args,
        } => {
            if let Some(function) = user_function(lowering, ctx, *pou) {
                emit_user_call(emitter, ctx, lowering, &function, args, expr.span())?;
            } else {
                // `can_emit` leaves to the AST a call of a function or
                // method this backend does not pass every argument of.
                let site = method_site(lowering, ctx, *pou, args).ok_or_else(|| {
                    Diagnostic::internal_error_at(Label::span(
                        expr.span().clone(),
                        "A call of a function this backend does not pass its arguments",
                    ))
                })?;
                emit_method_call(emitter, ctx, lowering, &site, &args[1..], expr.span())?;
                // Drop the instance from beneath the result.
                emitter.emit_swap();
                emitter.emit_pop();
            }
        }
        ExprKind::Call {
            callee: Callee::Intrinsic(intrinsic),
            args,
        } => emit_intrinsic_call(emitter, ctx, lowering, *intrinsic, args, expr.span())?,
        // `can_emit` leaves a statement with an in-place call to the AST.
        ExprKind::InPlaceCall(_) => return Err(in_place_not_emitted(expr.span())),
    }
    Ok(())
}

/// The internal error for an in-place call, which `can_emit` leaves to the
/// AST route.
pub(super) fn in_place_not_emitted(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "An in-place call this backend does not emit",
    ))
}

/// Applies `expr`, an operation of one operand, to the value of its operand
/// on the stack: a conversion, a narrowing or a unary operation.
pub(super) fn emit_operation(emitter: &mut Emitter, expr: &Expr) -> Result<(), Diagnostic> {
    let op = op_type(expr.ty()).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            expr.span().clone(),
            "A lowered expression has no operation type",
        ))
    })?;
    match expr.kind() {
        ExprKind::Convert(value) => {
            let from = op_type(value.ty()).unwrap_or(op);
            convert(emitter, from, op)?;
        }
        ExprKind::ConvertChecked(value) => {
            let from = op_type(value.ty()).unwrap_or(op);
            emit_subscript_check(emitter, from, op, expr.span())?;
        }
        ExprKind::Truncate { width, .. } => emit_truncate(emitter, *width, op.1, expr.span())?,
        ExprKind::Unary { op: unary, .. } => match unary {
            UnaryOp::Neg => emit_neg(emitter, op),
            UnaryOp::BitNot => match op.0 {
                OpWidth::W64 => emitter.emit_bit_not_64(),
                OpWidth::W32 | OpWidth::F32 | OpWidth::F64 => emitter.emit_bit_not_32(),
            },
            UnaryOp::BoolNot => emitter.emit_bool_not(),
        },
        ExprKind::Const(_)
        | ExprKind::Read(_)
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => {
            return Err(Diagnostic::internal_error_at(Label::span(
                expr.span().clone(),
                "A lowered expression that is not an operation of one operand",
            )))
        }
    }
    Ok(())
}

/// Calls the user-defined function `function` with `args`, the arguments it
/// takes for its inputs and in-outs, at `span`, leaving its result on the
/// stack: a value, or a string's buffer. As the route that reads the AST
/// does (`compile_user_function_call`), a string result's header is
/// initialized before `CALL`, for the function's body to write to.
pub(super) fn emit_user_call<'s>(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    function: &UserFunctionInfo,
    args: impl IntoIterator<Item = &'s Arg>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    emit_passed(emitter, ctx, lowering, &function.params, args, span)?;
    if let Some(result) = &function.return_string_info {
        emitter.emit_str_init(result.data_offset, result.max_length, result.char_width);
    }
    emitter.emit_call(
        function.function_id,
        function.num_params,
        function.var_offset,
        function.max_stack_depth,
    );
    ctx.record_call_edge(function.function_id);
    Ok(())
}

/// Emits the call of `intrinsic` with `args`, at `span`: its arguments, then
/// its instruction, which leaves a value or a temporary buffer on the stack.
pub(super) fn emit_intrinsic_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    intrinsic: Intrinsic,
    args: &[Arg],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    // `can_emit` leaves to the AST a call with no instruction.
    let instruction = instruction_of(intrinsic, args.len()).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "A call of an intrinsic with no instruction",
        ))
    })?;
    let strings = emit_args(emitter, ctx, lowering, args, span)?;
    match instruction {
        Instruction::Builtin(builtin) => emitter.emit_builtin(builtin)?,
        Instruction::BuiltinOfStrings(builtin) => {
            for string in strings {
                let offset = ctx.add_i32_constant(string as i32);
                emitter.emit_load_const_i32(offset);
            }
            emitter.emit_builtin(builtin)?;
        }
        Instruction::OneString(emit) => {
            let [string] = strings[..] else {
                return Err(strings_not_taken(span));
            };
            emit(emitter, string);
        }
        Instruction::TwoStrings(emit) => {
            let [in1, in2] = strings[..] else {
                return Err(strings_not_taken(span));
            };
            emit(emitter, in1, in2);
        }
    }
    Ok(())
}

/// Passes each of `args` of the call at `span`, in order: a value on the
/// stack, a string as the data region offset of a slot that holds it, and a
/// place by reference, as the AST route passes a `VAR_IN_OUT` argument.
/// Returns the offsets of the strings, in order.
fn emit_args<'s>(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    args: impl IntoIterator<Item = &'s Arg>,
    span: &SourceSpan,
) -> Result<Vec<u32>, Diagnostic> {
    let mut strings = Vec::new();
    for arg in args {
        match arg {
            Arg::Value(value) => emit_expr(emitter, ctx, lowering, value)?,
            Arg::Str(value) => strings.push(str_arg_offset(emitter, ctx, lowering, value)?),
            Arg::Ref(place) => emit_ref(emitter, ctx, lowering, place, span)?,
        }
    }
    Ok(strings)
}

/// Pushes a reference to `place`, a variable or what a `VAR_IN_OUT`
/// parameter refers to. `can_emit` leaves any other place to the AST.
pub(super) fn emit_ref(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    place: &Place,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let name = variable_name(lowering, ctx, place).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "A place this backend has no reference to",
        ))
    })?;
    emit_reference(emitter, ctx, &name)
}

/// The internal error for a call whose strings are not the ones its
/// instruction takes, which the IR's signature check rules out.
fn strings_not_taken(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A call of an intrinsic with strings its instruction does not take",
    ))
}

/// How this backend computes a call of an intrinsic. Each takes the call's
/// values on the stack.
#[derive(Clone, Copy, Debug)]
pub(super) enum Instruction {
    /// A VM builtin (ADR-0008).
    Builtin(u16),
    /// A VM builtin that also takes the data region offset of each of the
    /// call's strings, in order, as values after the call's values.
    BuiltinOfStrings(u16),
    /// A string opcode, emitted with the data region offset of the call's
    /// one string.
    OneString(fn(&mut Emitter, u32)),
    /// A string opcode, emitted with the data region offsets of the call's
    /// two strings.
    TwoStrings(fn(&mut Emitter, u32, u32)),
}

/// The instruction that computes a call of `intrinsic` with `args`
/// arguments; `None` for a `MUX` of more inputs than the VM has a builtin
/// for. The match names every intrinsic, so one the IR adds does not compile
/// until it has an instruction here (REQ-LOW-codegen-074).
pub(super) fn instruction_of(intrinsic: Intrinsic, args: usize) -> Option<Instruction> {
    // A `STRING_TO_*` has a builtin for each target and policy.
    let parse = |target, policy| Instruction::BuiltinOfStrings(parse_builtin(target, policy));
    // A `MUX` has a builtin for each number of inputs after its selector.
    let mux = |base: u16| {
        u16::try_from(args.saturating_sub(1))
            .ok()
            .filter(|inputs| *inputs <= builtin::MUX_MAX_INPUTS)
            .map(|inputs| Instruction::Builtin(base + inputs))
    };
    Some(Instruction::Builtin(match intrinsic {
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
        Intrinsic::ExptU32 => builtin::EXPT_U32,
        Intrinsic::ExptU64 => builtin::EXPT_U64,
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
        Intrinsic::LenString | Intrinsic::LenWString => {
            return Some(Instruction::OneString(Emitter::emit_len_str))
        }
        Intrinsic::FindString | Intrinsic::FindWString => {
            return Some(Instruction::TwoStrings(Emitter::emit_find_str))
        }
        Intrinsic::ConcatString | Intrinsic::ConcatWString => {
            return Some(Instruction::TwoStrings(Emitter::emit_concat_str))
        }
        Intrinsic::InsertString | Intrinsic::InsertWString => {
            return Some(Instruction::TwoStrings(Emitter::emit_insert_str))
        }
        Intrinsic::ReplaceString | Intrinsic::ReplaceWString => {
            return Some(Instruction::TwoStrings(Emitter::emit_replace_str))
        }
        Intrinsic::DeleteString | Intrinsic::DeleteWString => {
            return Some(Instruction::OneString(Emitter::emit_delete_str))
        }
        Intrinsic::LeftString | Intrinsic::LeftWString => {
            return Some(Instruction::OneString(Emitter::emit_left_str))
        }
        Intrinsic::RightString | Intrinsic::RightWString => {
            return Some(Instruction::OneString(Emitter::emit_right_str))
        }
        Intrinsic::MidString | Intrinsic::MidWString => {
            return Some(Instruction::OneString(Emitter::emit_mid_str))
        }
        Intrinsic::CompareString | Intrinsic::CompareWString => {
            return Some(Instruction::BuiltinOfStrings(builtin::CMP_STR))
        }
        Intrinsic::StringToU8(policy) => return Some(parse(Target::U8, policy)),
        Intrinsic::StringToI8(policy) => return Some(parse(Target::I8, policy)),
        Intrinsic::StringToU16(policy) => return Some(parse(Target::U16, policy)),
        Intrinsic::StringToI16(policy) => return Some(parse(Target::I16, policy)),
        Intrinsic::StringToU32(policy) => return Some(parse(Target::U32, policy)),
        Intrinsic::StringToI32(policy) => return Some(parse(Target::I32, policy)),
        Intrinsic::StringToU64(policy) => return Some(parse(Target::U64, policy)),
        Intrinsic::StringToI64(policy) => return Some(parse(Target::I64, policy)),
        Intrinsic::StringToF32(policy) => return Some(parse(Target::F32, policy)),
        Intrinsic::StringToF64(policy) => return Some(parse(Target::F64, policy)),
        Intrinsic::I32ToString => builtin::CONV_I32_TO_STR,
        Intrinsic::U32ToString => builtin::CONV_U32_TO_STR,
        Intrinsic::F32ToString => builtin::CONV_F32_TO_STR,
    }))
}

/// Narrows the value on the stack to `width` bits of `signedness`.
pub(super) fn emit_truncate(
    emitter: &mut Emitter,
    width: NarrowWidth,
    signedness: Signedness,
    span: &SourceSpan,
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
    fn instruction_of_when_mux_then_builtin_for_its_number_of_inputs() {
        let builtin_of = |intrinsic, args| match instruction_of(intrinsic, args) {
            Some(Instruction::Builtin(builtin)) => Some(builtin),
            Some(
                Instruction::BuiltinOfStrings(_)
                | Instruction::OneString(_)
                | Instruction::TwoStrings(_),
            )
            | None => None,
        };
        assert_eq!(
            builtin_of(Intrinsic::MuxF64, 3),
            Some(builtin::MUX_F64_BASE + 2)
        );
        assert_eq!(
            builtin_of(Intrinsic::MuxU32, 17),
            Some(builtin::MUX_I32_BASE + 16)
        );
        assert!(instruction_of(Intrinsic::MuxI64, 18).is_none());
    }
}
