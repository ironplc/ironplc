//! Standard functions the VM computes in one builtin chosen by the operand
//! type: the numeric functions ([`NumericFunction`]) and the bit shifts and
//! rotates ([`BitShift`]).
//!
//! The analyzer says which function a call is; this module says which
//! width-specific `BUILTIN` func_id computes it. Separated from
//! `compile_call.rs` to keep module sizes within the 1000-line guideline.

use ironplc_analyzer::{BitShift, Intrinsic, NumericFunction};
use ironplc_container::opcode;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr, Function};

use super::call_args::{collect_positional_args, wrong_arg_count};
use super::compile::{CompileContext, OpType, OpWidth, Signedness, DEFAULT_OP_TYPE};
use super::compile_expr::{compile_expr, op_type, storage_bits};
use crate::emit::Emitter;

/// Builds the opcode for a builtin defined across all four operation widths
/// (both integer widths and both float widths), independent of signedness.
///
/// Used for the identically-shaped EXPT/ABS/SEL arms of [`lookup_builtin`].
macro_rules! numeric_builtin {
    ($op_width:expr, $i32_op:path, $i64_op:path, $f32_op:path, $f64_op:path) => {
        Some(match $op_width {
            OpWidth::W32 => $i32_op,
            OpWidth::W64 => $i64_op,
            OpWidth::F32 => $f32_op,
            OpWidth::F64 => $f64_op,
        })
    };
}

/// Builds the opcode for a builtin whose integer variants distinguish
/// signedness but whose float variants do not.
///
/// Used for the identically-shaped MIN/MAX/LIMIT arms of [`lookup_builtin`].
macro_rules! signed_numeric_builtin {
    ($op_width:expr, $signedness:expr,
     $i32_op:path, $u32_op:path, $i64_op:path, $u64_op:path,
     $f32_op:path, $f64_op:path) => {
        Some(match ($op_width, $signedness) {
            (OpWidth::W32, Signedness::Signed) => $i32_op,
            (OpWidth::W32, Signedness::Unsigned) => $u32_op,
            (OpWidth::W64, Signedness::Signed) => $i64_op,
            (OpWidth::W64, Signedness::Unsigned) => $u64_op,
            (OpWidth::F32, _) => $f32_op,
            (OpWidth::F64, _) => $f64_op,
        })
    };
}

/// Builds the opcode for a float-only (transcendental) builtin. The integer
/// operation widths have no variant and yield `None`.
///
/// Used for the many identically-shaped SQRT/LN/.../ATAN2 arms of
/// [`lookup_builtin`].
macro_rules! float_builtin {
    ($op_width:expr, $f32_op:path, $f64_op:path) => {
        match $op_width {
            OpWidth::F32 => Some($f32_op),
            OpWidth::F64 => Some($f64_op),
            OpWidth::W32 | OpWidth::W64 => None,
        }
    };
}

/// Returns the builtin opcode for a numeric standard function at an
/// operation type, or `None` when the function has no builtin at that width.
///
/// The `op_width` selects the correct width variant and `signedness` selects
/// the signed/unsigned variant for functions that distinguish them.
///
/// The arms delegate to the `*_builtin!` macros above, which each capture one
/// recurring arm shape (all-widths, signed/unsigned, float-only). The opcode
/// identifiers are still written verbatim per arm — the workspace has no
/// `paste` crate to concatenate them, and spelling them out keeps every opcode
/// greppable — while the macros remove the ~55 lines of duplicated `match`
/// scaffolding the arms would otherwise repeat.
pub(crate) fn lookup_builtin(
    function: NumericFunction,
    op_width: OpWidth,
    signedness: Signedness,
) -> Option<u16> {
    use opcode::builtin;
    match function {
        NumericFunction::Expt => numeric_builtin!(
            op_width,
            builtin::EXPT_I32,
            builtin::EXPT_I64,
            builtin::EXPT_F32,
            builtin::EXPT_F64
        ),
        NumericFunction::Abs => numeric_builtin!(
            op_width,
            builtin::ABS_I32,
            builtin::ABS_I64,
            builtin::ABS_F32,
            builtin::ABS_F64
        ),
        NumericFunction::Sel => numeric_builtin!(
            op_width,
            builtin::SEL_I32,
            builtin::SEL_I64,
            builtin::SEL_F32,
            builtin::SEL_F64
        ),
        NumericFunction::Min => signed_numeric_builtin!(
            op_width,
            signedness,
            builtin::MIN_I32,
            builtin::MIN_U32,
            builtin::MIN_I64,
            builtin::MIN_U64,
            builtin::MIN_F32,
            builtin::MIN_F64
        ),
        NumericFunction::Max => signed_numeric_builtin!(
            op_width,
            signedness,
            builtin::MAX_I32,
            builtin::MAX_U32,
            builtin::MAX_I64,
            builtin::MAX_U64,
            builtin::MAX_F32,
            builtin::MAX_F64
        ),
        NumericFunction::Limit => signed_numeric_builtin!(
            op_width,
            signedness,
            builtin::LIMIT_I32,
            builtin::LIMIT_U32,
            builtin::LIMIT_I64,
            builtin::LIMIT_U64,
            builtin::LIMIT_F32,
            builtin::LIMIT_F64
        ),
        NumericFunction::Sqrt => float_builtin!(op_width, builtin::SQRT_F32, builtin::SQRT_F64),
        NumericFunction::Ln => float_builtin!(op_width, builtin::LN_F32, builtin::LN_F64),
        NumericFunction::Log => float_builtin!(op_width, builtin::LOG_F32, builtin::LOG_F64),
        NumericFunction::Exp => float_builtin!(op_width, builtin::EXP_F32, builtin::EXP_F64),
        NumericFunction::Sin => float_builtin!(op_width, builtin::SIN_F32, builtin::SIN_F64),
        NumericFunction::Cos => float_builtin!(op_width, builtin::COS_F32, builtin::COS_F64),
        NumericFunction::Tan => float_builtin!(op_width, builtin::TAN_F32, builtin::TAN_F64),
        NumericFunction::Asin => float_builtin!(op_width, builtin::ASIN_F32, builtin::ASIN_F64),
        NumericFunction::Acos => float_builtin!(op_width, builtin::ACOS_F32, builtin::ACOS_F64),
        NumericFunction::Atan => float_builtin!(op_width, builtin::ATAN_F32, builtin::ATAN_F64),
        NumericFunction::Atan2 => {
            float_builtin!(op_width, builtin::ATAN2_F32, builtin::ATAN2_F64)
        }
        // Compiler intrinsics (reserved `__` namespace): real-preserving
        // truncation and floating modulo, ANY_REAL with the width selecting
        // the F32/F64 builtin variant.
        NumericFunction::TruncReal => {
            float_builtin!(op_width, builtin::TRUNC_F32, builtin::TRUNC_F64)
        }
        NumericFunction::ModReal => float_builtin!(op_width, builtin::MOD_F32, builtin::MOD_F64),
    }
}

/// Compiles a call to a numeric function via [`lookup_builtin`] at
/// `op_type`, which selects the builtin.
///
/// A function of several inputs of one type (`MIN`, `MAX`, `LIMIT`, `SEL`,
/// `EXPT`, `ATAN2`) computes at the type the analyzer recorded for the call,
/// and compiles each argument at the type it recorded for that: an input of
/// the function's type at `op_type`'s width, to which it converted one of
/// another width, and `SEL`'s selector as the `BOOL` it is (ADR-0056). A
/// function of one input compiles it at `op_type`.
pub(crate) fn compile_numeric(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    function: NumericFunction,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let func_id = lookup_builtin(function, op_type.0, op_type.1)
        .ok_or_else(|| Diagnostic::todo_with_span(func.name.span()))?;

    let expected_args =
        opcode::builtin::arg_count_opt(func_id).ok_or_else(Diagnostic::internal_error)? as usize;

    let args = collect_positional_args(func);

    if args.len() != expected_args {
        return Err(wrong_arg_count(func));
    }

    let of_one_type = Intrinsic::Numeric(function).inputs_of_one_type().is_some();
    for arg in &args {
        let arg_op_type = if of_one_type {
            self::op_type(ctx, arg)?
        } else {
            op_type
        };
        compile_expr(emitter, ctx, arg, arg_op_type)?;
    }

    emitter.emit_builtin(func_id)?;
    Ok(())
}

/// Returns the builtin opcode for the shift or rotate `shift` at `op_width`,
/// on an operand of `bits` value bits.
///
/// A rotate of a BYTE or WORD has its own builtin so that bits wrap around
/// within the narrow type rather than within the 32-bit slot.
pub(crate) fn shift_builtin(shift: BitShift, op_width: OpWidth, bits: u8) -> u16 {
    use opcode::builtin;
    match (shift, op_width) {
        (BitShift::Shl, OpWidth::W64) => builtin::SHL_I64,
        (BitShift::Shl, _) => builtin::SHL_I32,
        (BitShift::Shr, OpWidth::W64) => builtin::SHR_I64,
        (BitShift::Shr, _) => builtin::SHR_I32,
        (BitShift::Rol, OpWidth::W64) => builtin::ROL_I64,
        (BitShift::Rol, _) => match bits {
            8 => builtin::ROL_U8,
            16 => builtin::ROL_U16,
            _ => builtin::ROL_I32,
        },
        (BitShift::Ror, OpWidth::W64) => builtin::ROR_I64,
        (BitShift::Ror, _) => match bits {
            8 => builtin::ROR_U8,
            16 => builtin::ROR_U16,
            _ => builtin::ROR_I32,
        },
    }
}

/// Compiles a bit shift or rotate function call (SHL, SHR, ROL, ROR).
///
/// Expects two positional arguments: IN (value) and N (shift count).
/// Emits the builtin [`shift_builtin`] selects for the operand width.
pub(crate) fn compile_shift_rotate(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 2],
    op_type: OpType,
    shift: BitShift,
) -> Result<(), Diagnostic> {
    // Compile IN (value) with the inferred op_type
    compile_expr(emitter, ctx, args[0], op_type)?;
    // Compile N (shift count) — always as i32 for W32, i64 for W64
    let n_op_type = match op_type.0 {
        OpWidth::W64 => (OpWidth::W64, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    };
    compile_expr(emitter, ctx, args[1], n_op_type)?;

    // Determine storage bits for narrow-type ROL/ROR selection
    let bits = storage_bits(ctx, args[0])?;

    emitter.emit_builtin(shift_builtin(shift, op_type.0, bits))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opcode::builtin;
    use rstest::rstest;

    /// Every numeric function, at a width it has a builtin for.
    #[rstest]
    #[case::abs(NumericFunction::Abs, OpWidth::W32, builtin::ABS_I32)]
    #[case::sqrt(NumericFunction::Sqrt, OpWidth::F32, builtin::SQRT_F32)]
    #[case::ln(NumericFunction::Ln, OpWidth::F64, builtin::LN_F64)]
    #[case::log(NumericFunction::Log, OpWidth::F32, builtin::LOG_F32)]
    #[case::exp(NumericFunction::Exp, OpWidth::F64, builtin::EXP_F64)]
    #[case::sin(NumericFunction::Sin, OpWidth::F32, builtin::SIN_F32)]
    #[case::cos(NumericFunction::Cos, OpWidth::F64, builtin::COS_F64)]
    #[case::tan(NumericFunction::Tan, OpWidth::F32, builtin::TAN_F32)]
    #[case::asin(NumericFunction::Asin, OpWidth::F64, builtin::ASIN_F64)]
    #[case::acos(NumericFunction::Acos, OpWidth::F32, builtin::ACOS_F32)]
    #[case::atan(NumericFunction::Atan, OpWidth::F64, builtin::ATAN_F64)]
    #[case::atan2(NumericFunction::Atan2, OpWidth::F64, builtin::ATAN2_F64)]
    #[case::expt(NumericFunction::Expt, OpWidth::W64, builtin::EXPT_I64)]
    #[case::min(NumericFunction::Min, OpWidth::W32, builtin::MIN_I32)]
    #[case::max(NumericFunction::Max, OpWidth::W64, builtin::MAX_I64)]
    #[case::limit(NumericFunction::Limit, OpWidth::F32, builtin::LIMIT_F32)]
    #[case::sel(NumericFunction::Sel, OpWidth::F64, builtin::SEL_F64)]
    #[case::trunc_real(NumericFunction::TruncReal, OpWidth::F32, builtin::TRUNC_F32)]
    #[case::mod_real(NumericFunction::ModReal, OpWidth::F64, builtin::MOD_F64)]
    fn lookup_builtin_when_numeric_function_then_its_builtin(
        #[case] function: NumericFunction,
        #[case] op_width: OpWidth,
        #[case] expected: u16,
    ) {
        assert_eq!(
            lookup_builtin(function, op_width, Signedness::Signed),
            Some(expected)
        );
    }

    #[test]
    fn lookup_builtin_when_all_width_numeric_then_selects_by_width() {
        // EXPT/ABS/SEL are defined for every op width, independent of sign.
        // An unsigned ABS never reaches code generation: the analyzer
        // removes it (`xform_remove_unsigned_abs`).
        assert_eq!(
            lookup_builtin(NumericFunction::Abs, OpWidth::W32, Signedness::Signed),
            Some(builtin::ABS_I32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Abs, OpWidth::W64, Signedness::Signed),
            Some(builtin::ABS_I64)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Expt, OpWidth::F32, Signedness::Signed),
            Some(builtin::EXPT_F32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Sel, OpWidth::F64, Signedness::Signed),
            Some(builtin::SEL_F64)
        );
    }

    #[test]
    fn lookup_builtin_when_signed_numeric_then_selects_by_width_and_sign() {
        assert_eq!(
            lookup_builtin(NumericFunction::Min, OpWidth::W32, Signedness::Signed),
            Some(builtin::MIN_I32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Min, OpWidth::W32, Signedness::Unsigned),
            Some(builtin::MIN_U32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Max, OpWidth::W64, Signedness::Unsigned),
            Some(builtin::MAX_U64)
        );
        // Float variants ignore signedness.
        assert_eq!(
            lookup_builtin(NumericFunction::Limit, OpWidth::F32, Signedness::Unsigned),
            Some(builtin::LIMIT_F32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Limit, OpWidth::F64, Signedness::Signed),
            Some(builtin::LIMIT_F64)
        );
    }

    #[test]
    fn lookup_builtin_when_float_only_and_float_width_then_selects_variant() {
        assert_eq!(
            lookup_builtin(NumericFunction::Sin, OpWidth::F32, Signedness::Signed),
            Some(builtin::SIN_F32)
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Atan2, OpWidth::F64, Signedness::Signed),
            Some(builtin::ATAN2_F64)
        );
    }

    #[test]
    fn lookup_builtin_when_float_only_and_integer_width_then_none() {
        assert_eq!(
            lookup_builtin(NumericFunction::Sin, OpWidth::W32, Signedness::Signed),
            None
        );
        assert_eq!(
            lookup_builtin(NumericFunction::Sqrt, OpWidth::W64, Signedness::Unsigned),
            None
        );
        assert_eq!(
            lookup_builtin(NumericFunction::ModReal, OpWidth::W32, Signedness::Signed),
            None
        );
    }

    #[rstest]
    #[case::shl_32(BitShift::Shl, OpWidth::W32, 32, builtin::SHL_I32)]
    #[case::shl_byte(BitShift::Shl, OpWidth::W32, 8, builtin::SHL_I32)]
    #[case::shl_64(BitShift::Shl, OpWidth::W64, 64, builtin::SHL_I64)]
    #[case::shr_word(BitShift::Shr, OpWidth::W32, 16, builtin::SHR_I32)]
    #[case::shr_64(BitShift::Shr, OpWidth::W64, 64, builtin::SHR_I64)]
    #[case::rol_byte(BitShift::Rol, OpWidth::W32, 8, builtin::ROL_U8)]
    #[case::rol_word(BitShift::Rol, OpWidth::W32, 16, builtin::ROL_U16)]
    #[case::rol_dword(BitShift::Rol, OpWidth::W32, 32, builtin::ROL_I32)]
    #[case::rol_64(BitShift::Rol, OpWidth::W64, 64, builtin::ROL_I64)]
    #[case::ror_byte(BitShift::Ror, OpWidth::W32, 8, builtin::ROR_U8)]
    #[case::ror_word(BitShift::Ror, OpWidth::W32, 16, builtin::ROR_U16)]
    #[case::ror_dword(BitShift::Ror, OpWidth::W32, 32, builtin::ROR_I32)]
    #[case::ror_64(BitShift::Ror, OpWidth::W64, 64, builtin::ROR_I64)]
    fn shift_builtin_when_shift_then_selects_by_width_and_bits(
        #[case] shift: BitShift,
        #[case] op_width: OpWidth,
        #[case] bits: u8,
        #[case] expected: u16,
    ) {
        assert_eq!(shift_builtin(shift, op_width, bits), expected);
    }
}
