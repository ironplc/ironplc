//! Function call compilation for IEC 61131-3 code generation.
//!
//! Contains standard library function dispatch, type conversions and time
//! functions; a call to a user-defined function is `compile_user_call.rs`. Separated from compile.rs to keep
//! module sizes within the 1000-line guideline.

use std::collections::HashMap;

use ironplc_analyzer::{
    BuiltinFunction, CounterWidth, FormOf, FunctionEnvironment, StandardFunctionBlock,
    StringFunction, TypeEnvironment,
};
use ironplc_container::{opcode, FbTypeId};
use ironplc_dsl::common::{ElementaryTypeName, TypeName};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, Function, SymbolicVariableKind, Variable};

use super::call_args::{collect_input_args, fixed_args, wrong_arg_count};
use super::compile::{CompileContext, OpType, OpWidth, Signedness, VarTypeInfo, NARROW_CHAR_WIDTH};
use super::compile_arith::compile_arith_fold;
use super::compile_builtin::{compile_numeric, compile_shift_rotate};
use super::compile_comparison::compile_comparison;
use super::compile_expr::{
    compile_expr, emit_compare_op, emit_mod, emit_mul, emit_not, emit_sub, emit_truncation,
    op_type, storage_bits,
};
use super::compile_string::{
    compile_concat, compile_delete, compile_find, compile_insert, compile_left, compile_len,
    compile_mid, compile_replace, compile_right, resolve_string_arg,
};
use super::compile_time_arith::{compile_time_arith, time_arith_for};
use super::type_info::{elementary_type_info, expr_operand_name};
use crate::emit::Emitter;

/// Returns the operation each standard function in `functions` stands for,
/// by name.
///
/// Codegen recognizes a standard function only through this table, built
/// from the signatures the analyzer resolved calls against, never by its
/// spelling.
pub(crate) fn builtin_functions_by_name(
    functions: &FunctionEnvironment,
) -> HashMap<Id, BuiltinFunction> {
    functions
        .iter()
        .filter_map(|(_, signature)| {
            Some((signature.name.clone(), signature.builtin_function.clone()?))
        })
        .collect()
}

/// Compiles `call`, the call `func`, whose own operation type is `at`.
///
/// A standard function compiles as the operation its signature names; any
/// other function is a user-defined one.
pub(crate) fn compile_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &Expr,
    func: &Function,
    at: OpType,
) -> Result<(), Diagnostic> {
    if let Some(builtin) = ctx.builtin_functions.get(&func.name).cloned() {
        return compile_intrinsic(emitter, ctx, call, func, at, builtin);
    }
    let Some(func_info) = ctx.user_function(func.name.lower_case()).cloned() else {
        return Err(Diagnostic::todo_with_span(func.name.span()));
    };
    super::compile_user_call::compile_user_function_call(emitter, ctx, func, &func_info)
}

/// Compiles `call`, a call to a standard function, as the operation
/// `builtin` at `at`, the call's own operation type.
///
/// The type it computes at is the one the analyzer recorded for the call: for
/// a function of several inputs of one type (`MAX`, `MUX`, ...), the type it
/// converted each input to (ADR-0056); for an operation on one value, its
/// operand's; and for `TRUNC`, `BCD_TO_INT` and `SIZEOF`, whose integer
/// result takes the type of its context, that type. A call of one of these
/// without a type is one the analyzer did not resolve.
fn compile_intrinsic(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &Expr,
    func: &Function,
    at: OpType,
    builtin: BuiltinFunction,
) -> Result<(), Diagnostic> {
    let result = expr_operand_name(ctx, call);
    let result = result.as_ref();
    if builtin.computes_at_own_type() || builtin.result_is_integer_of_context() {
        let at = self::op_type(ctx, call)?;
        return compile_intrinsic_at(emitter, ctx, func, result, at, builtin);
    }
    compile_intrinsic_at(emitter, ctx, func, result, at, builtin)
}

/// Compiles a call to a standard function as the operation `builtin`, at
/// `op_type`.
///
/// The match has an arm for every operation, so a standard function the
/// analyzer adds does not compile until it has one here.
fn compile_intrinsic_at(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    result: Option<&TypeName>,
    op_type: OpType,
    builtin: BuiltinFunction,
) -> Result<(), Diagnostic> {
    match builtin {
        // A function form of an operator (ADD, GT, AND, NOT, ...) compiles
        // as the operator it is a form of.
        BuiltinFunction::Operator(operator) => {
            compile_operator_form(emitter, ctx, func, result, op_type, &operator)
        }
        BuiltinFunction::Numeric(function) => {
            compile_numeric(emitter, ctx, func, function, op_type)
        }
        BuiltinFunction::BitShift(shift) => {
            compile_shift_rotate(emitter, ctx, fixed_args(func)?, op_type, shift)
        }
        BuiltinFunction::Mux => compile_mux(emitter, ctx, func, op_type),
        // Assignment function (equivalent to := operator)
        BuiltinFunction::Move => compile_move(emitter, ctx, fixed_args(func)?),
        BuiltinFunction::Trunc => compile_trunc(emitter, ctx, fixed_args(func)?, op_type),
        BuiltinFunction::BcdToInt => {
            compile_bcd_to_int(emitter, ctx, fixed_args(func)?, &func.name.span(), op_type)
        }
        BuiltinFunction::IntToBcd => {
            compile_int_to_bcd(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        // SIZEOF operator (extension)
        BuiltinFunction::Sizeof => compile_sizeof(emitter, ctx, fixed_args(func)?, op_type),
        BuiltinFunction::String(StringFunction::Len) => {
            compile_len(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Find) => {
            compile_find(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Replace) => {
            compile_replace(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Insert) => {
            compile_insert(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Delete) => {
            compile_delete(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Left) => {
            compile_left(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Right) => {
            compile_right(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Mid) => {
            compile_mid(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::String(StringFunction::Concat) => {
            compile_concat(emitter, ctx, fixed_args(func)?, &func.name.span())
        }
        BuiltinFunction::Conversion { source, target } => compile_conversion(
            emitter,
            ctx,
            fixed_args(func)?,
            &func.name.span(),
            &source,
            &target,
        ),
        // A typed time or date function (ADD_TIME, SUB_DATE_DATE, ...)
        // compiles as the instruction sequence for the units of its operands.
        BuiltinFunction::Time { function, long } => {
            let (arith, width) = time_arith_for(function, long);
            let [in1, in2] = fixed_args::<2>(func)?;
            compile_time_arith(emitter, ctx, arith, width, in1, in2)
        }
        // Time functions: datetime decomposition
        BuiltinFunction::DtToDate => compile_dt_to_date(emitter, ctx, fixed_args(func)?),
        BuiltinFunction::DtToTod => compile_dt_to_tod(emitter, ctx, fixed_args(func)?),
    }
}

/// Compiles the function form of an operator as the operator itself.
///
/// Each argument compiles at its own type, and the operator computes at
/// `op_type`, the call's own type (see [`compile_intrinsic`]); the opcode
/// comes from the emitter the operator expression uses, so `AND(a, b)` and
/// `a AND b` cannot diverge.
///
/// A binary operator folds its arguments from the left, so `ADD(a, b, c)`
/// compiles as `(a + b) + c`. The analyzer has already enforced how many
/// arguments the form takes; the fold is the same code for the two of a
/// binary form and the two or more of an extensible one.
fn compile_operator_form(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    result: Option<&TypeName>,
    op_type: OpType,
    operator: &FormOf,
) -> Result<(), Diagnostic> {
    match operator {
        FormOf::Arithmetic(op) => compile_arith_fold(emitter, ctx, func, op, result),
        // A comparison computes at the type of its operands, not at the
        // enclosing `op_type`, which is the type of the BOOL it yields.
        FormOf::Compare(op) if op.is_comparison() => {
            let [left, right] = fixed_args::<2>(func)?;
            compile_comparison(emitter, ctx, op, left, right)
        }
        FormOf::Compare(op) => {
            compile_left_fold(emitter, ctx, func, op_type, |emitter, op_type| {
                emit_compare_op(emitter, op, op_type);
                Ok(())
            })
        }
        FormOf::Not => {
            let [term] = fixed_args::<1>(func)?;
            compile_expr(emitter, ctx, term)?;
            emit_not(emitter, ctx, op_type, term)
        }
    }
}

/// Compiles a call's two or more positional arguments, emitting the operator
/// after each argument but the first, so the arguments fold from the left.
pub(crate) fn compile_left_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
    emit_fn: impl Fn(&mut Emitter, OpType) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let args = collect_input_args(func);
    let [first, rest @ ..] = args.as_slice() else {
        return Err(wrong_arg_count(func));
    };
    if rest.is_empty() {
        return Err(wrong_arg_count(func));
    }
    compile_expr(emitter, ctx, first)?;
    for arg in rest {
        compile_expr(emitter, ctx, arg)?;
        emit_fn(emitter, op_type)?;
    }
    Ok(())
}

/// Compiles DT_TO_DATE and DATE_AND_TIME_TO_DATE.
///
/// Extracts the date portion from a DATE_AND_TIME by stripping the
/// time-of-day: `IN - (IN % 86400)`. Both DT and DATE are in seconds
/// since 1970-01-01.
fn compile_dt_to_date(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
) -> Result<(), Diagnostic> {
    let op_type = (OpWidth::W32, Signedness::Unsigned);
    // Stack: IN
    compile_expr(emitter, ctx, args[0])?;
    // Stack: IN, IN
    compile_expr(emitter, ctx, args[0])?;
    // Stack: IN, IN, 86400
    let secs_per_day = ctx.add_i32_constant(86400);
    emitter.emit_load_const_i32(secs_per_day);
    // Stack: IN, (IN % 86400)
    emit_mod(emitter, op_type);
    // Stack: IN - (IN % 86400)
    emit_sub(emitter, op_type);
    Ok(())
}

/// Compiles DT_TO_TOD and DATE_AND_TIME_TO_TIME_OF_DAY.
///
/// Extracts the time-of-day from a DATE_AND_TIME: `(IN % 86400) * 1000`.
/// DT is in seconds since epoch; TOD is in milliseconds since midnight.
fn compile_dt_to_tod(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
) -> Result<(), Diagnostic> {
    let op_type = (OpWidth::W32, Signedness::Unsigned);
    // Stack: IN
    compile_expr(emitter, ctx, args[0])?;
    // Stack: IN, 86400
    let secs_per_day = ctx.add_i32_constant(86400);
    emitter.emit_load_const_i32(secs_per_day);
    // Stack: (IN % 86400)
    emit_mod(emitter, op_type);
    // Stack: (IN % 86400), 1000
    let ms_per_sec = ctx.add_i32_constant(1000);
    emitter.emit_load_const_i32(ms_per_sec);
    // Stack: (IN % 86400) * 1000
    emit_mul(emitter, op_type);
    Ok(())
}

/// Compiles the MOVE function form.
///
/// MOVE(IN) is equivalent to assignment. Takes a single argument and returns
/// it unchanged. No opcode is needed since the value is already on the stack.
fn compile_move(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
) -> Result<(), Diagnostic> {
    compile_expr(emitter, ctx, args[0])?;
    // No additional opcode needed - the value is already on the stack

    Ok(())
}

/// Compiles TRUNC(IN) — truncates a real value toward zero to an integer of
/// `target_op_type`, the type the analyzer recorded for the call.
///
/// The argument is compiled using its own (float) op_type derived from the
/// argument's resolved type. The result is converted to the target integer
/// type using the existing conversion opcodes.
fn compile_trunc(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    target_op_type: OpType,
) -> Result<(), Diagnostic> {
    // Determine the argument's float type from its resolved type.
    let arg_op_type = op_type(ctx, args[0])?;
    compile_expr(emitter, ctx, args[0])?;

    // Build VarTypeInfo for source (float) and target (integer) to reuse
    // the existing conversion opcode emission.
    let source = VarTypeInfo {
        op_width: arg_op_type.0,
        signedness: arg_op_type.1,
        storage_bits: match arg_op_type.0 {
            OpWidth::F32 => 32,
            OpWidth::F64 => 64,
            _ => 32,
        },
    };
    let target = VarTypeInfo {
        op_width: target_op_type.0,
        signedness: target_op_type.1,
        storage_bits: match target_op_type.0 {
            OpWidth::W32 => 32,
            OpWidth::W64 => 64,
            _ => 32,
        },
    };
    emit_conversion_opcode(emitter, &source, &target)?;

    Ok(())
}

/// Compiles SIZEOF(IN) — returns the size in bytes of the argument's type.
///
/// SIZEOF is a compile-time constant: the argument is never evaluated at runtime.
/// For elementary types, the size is derived from the storage bit width.
/// For array variables, the total byte count is computed from element size × element count.
fn compile_sizeof(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    op_type: OpType,
) -> Result<(), Diagnostic> {
    // Check if the argument is a variable that maps to an array.
    let size: u32 =
        if let ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(ref named))) =
            args[0].kind
        {
            if let Some(array_info) = ctx.array_var(&named.name) {
                // Ceiling division, as for a scalar: a BOOL element occupies 1 byte.
                let elem_bytes = (array_info.element_var_type_info.storage_bits as u32).div_ceil(8);
                array_info.total_elements * elem_bytes
            } else {
                sizeof_from_expr_type(ctx, args[0])?
            }
        } else {
            sizeof_from_expr_type(ctx, args[0])?
        };

    // The size is pushed at the integer type the analyzer recorded for the
    // call, which is the type of its context.
    match op_type.0 {
        OpWidth::W64 => {
            let pool_index = ctx.add_i64_constant(i64::from(size));
            emitter.emit_load_const_i64(pool_index);
        }
        OpWidth::W32 | OpWidth::F32 | OpWidth::F64 => {
            let pool_index = ctx.add_i32_constant(size as i32);
            emitter.emit_load_const_i32(pool_index);
        }
    }
    Ok(())
}

/// Returns the size in bytes of an expression's value, from its `expr_type`.
fn sizeof_from_expr_type(ctx: &CompileContext, expr: &Expr) -> Result<u32, Diagnostic> {
    let bits = storage_bits(ctx, expr)?;
    // Ceiling division: types like BOOL (1 bit) still occupy 1 byte.
    Ok((bits as u32).div_ceil(8))
}

/// Compiles BCD_TO_INT(IN) — converts a BCD-encoded bit string to an integer
/// of `target_op_type`, the type the analyzer recorded for the call.
///
/// The argument is compiled using its own (bit-string) op_type. The BCD
/// decoding opcode is selected based on the argument's storage bit width.
fn compile_bcd_to_int(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    span: &SourceSpan,
    target_op_type: OpType,
) -> Result<(), Diagnostic> {
    let bits = storage_bits(ctx, args[0])?;
    compile_expr(emitter, ctx, args[0])?;

    let func_id = match bits {
        8 => opcode::builtin::BCD_TO_INT_8,
        16 => opcode::builtin::BCD_TO_INT_16,
        32 => opcode::builtin::BCD_TO_INT_32,
        64 => opcode::builtin::BCD_TO_INT_64,
        _ => return Err(Diagnostic::todo_with_span(span.clone())),
    };
    emitter.emit_builtin(func_id)?;
    // The builtin decodes into a 64-bit integer for a 64-bit input and a
    // 32-bit one otherwise; the result is the integer type the analyzer
    // recorded for the call.
    let decoded = match bits {
        64 => (OpWidth::W64, Signedness::Signed),
        _ => (OpWidth::W32, Signedness::Signed),
    };
    crate::compile_arith::convert(emitter, decoded, target_op_type)?;
    Ok(())
}

/// Compiles INT_TO_BCD(IN) — converts an integer to a BCD-encoded bit string.
///
/// The argument is compiled at its own (integer) type, and the BCD encoding
/// opcode is selected by its width: the call computes at the bit string as
/// wide as its input. An input of a width with no encoding opcode is one the
/// analyzer did not type, and is an internal error rather than encoded at the
/// width of the call's context.
fn compile_int_to_bcd(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let arg_op_type = op_type(ctx, args[0])?;
    let bits = storage_bits(ctx, args[0])?;
    compile_expr(emitter, ctx, args[0])?;

    let func_id = match (arg_op_type.0, bits) {
        (OpWidth::W32, 8) => opcode::builtin::INT_TO_BCD_8,
        (OpWidth::W32, 16) => opcode::builtin::INT_TO_BCD_16,
        (OpWidth::W32, 32) => opcode::builtin::INT_TO_BCD_32,
        (OpWidth::W64, 64) => opcode::builtin::INT_TO_BCD_64,
        (width, bits) => {
            return Err(Diagnostic::internal_error_at(Label::span(
                span.clone(),
                format!("INT_TO_BCD of an input of width {width:?} and {bits} bits"),
            )));
        }
    };
    emitter.emit_builtin(func_id)?;
    Ok(())
}

/// Compiles a MUX (multiplexer) function call at `op_type`, the type the
/// analyzer recorded for the call.
///
/// MUX(K, IN0, IN1, ..., INn) selects one of the IN values based on the
/// integer selector K. Each argument compiles at the type the analyzer
/// recorded for it: K at a 32-bit integer type, to which it converted a
/// wider one, and each IN at `op_type`'s width (ADR-0056).
///
/// The opcode encodes the number of IN arguments: `MUX_<WIDTH>_BASE + num_inputs`.
fn compile_mux(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args = collect_input_args(func);

    // Must have at least 3 args (K + 2 IN values)
    if args.len() < 3 {
        return Err(wrong_arg_count(func));
    }

    let num_inputs = (args.len() - 1) as u16; // subtract K

    if num_inputs > opcode::builtin::MUX_MAX_INPUTS {
        return Err(wrong_arg_count(func));
    }

    let base = match op_type.0 {
        OpWidth::W32 => opcode::builtin::MUX_I32_BASE,
        OpWidth::W64 => opcode::builtin::MUX_I64_BASE,
        OpWidth::F32 => opcode::builtin::MUX_F32_BASE,
        OpWidth::F64 => opcode::builtin::MUX_F64_BASE,
    };
    let func_id = base + num_inputs;

    for arg in &args {
        compile_expr(emitter, ctx, arg)?;
    }

    emitter.emit_builtin(func_id)?;
    Ok(())
}

/// Compiles a type conversion function call (e.g., INT_TO_REAL).
///
/// Unlike generic builtins, conversion functions have different source and
/// target types. The argument is compiled with the source type's OpType,
/// then a conversion opcode (if needed) transforms the value to the target
/// representation.
fn compile_type_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    span: &SourceSpan,
    source: VarTypeInfo,
    target: VarTypeInfo,
) -> Result<(), Diagnostic> {
    compile_expr(emitter, ctx, args[0])?;

    // Integer-to-boolean needs a dedicated opcode (non-zero → 1, zero → 0)
    // rather than the generic conversion + truncation path, because
    // truncation would only keep the lowest bit instead of testing for zero.
    if target.storage_bits == 1 {
        match source.op_width {
            OpWidth::W32 => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_BOOL)?,
            OpWidth::W64 => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_BOOL)?,
            _ => {
                return Err(Diagnostic::internal_error_at(Label::span(
                    span.clone(),
                    "Boolean conversion from a source that is not 32 or 64 bits wide",
                )));
            }
        }
    } else {
        emit_conversion_opcode(emitter, &source, &target)?;
        emit_truncation(emitter, target);
    }

    Ok(())
}

/// Emits the appropriate conversion BUILTIN opcode for the source->target
/// type transition. Does nothing for same-domain integer conversions that
/// are handled by the Slot's sign-extension and truncation.
pub(crate) fn emit_conversion_opcode(
    emitter: &mut Emitter,
    source: &VarTypeInfo,
    target: &VarTypeInfo,
) -> Result<(), Diagnostic> {
    use opcode::builtin::*;
    use OpWidth::*;
    use Signedness::*;

    let func_id = match (
        source.op_width,
        source.signedness,
        target.op_width,
        target.signedness,
    ) {
        // Same OpWidth: no conversion needed (truncation handles sub-width)
        (W32, _, W32, _) | (W64, _, W64, _) => None,

        // W32 signed -> W64: sign extension already in Slot, no-op
        (W32, Signed, W64, _) => None,

        // W32 unsigned -> W64: need zero-extension
        (W32, Unsigned, W64, _) => Some(CONV_U32_TO_I64),

        // W64 -> W32: as_i32() truncation at store time, no-op
        (W64, _, W32, _) => None,

        // Integer -> Float
        (W32, Signed, F32, _) => Some(CONV_I32_TO_F32),
        (W32, Signed, F64, _) => Some(CONV_I32_TO_F64),
        (W64, Signed, F32, _) => Some(CONV_I64_TO_F32),
        (W64, Signed, F64, _) => Some(CONV_I64_TO_F64),
        (W32, Unsigned, F32, _) => Some(CONV_U32_TO_F32),
        (W32, Unsigned, F64, _) => Some(CONV_U32_TO_F64),
        (W64, Unsigned, F32, _) => Some(CONV_U64_TO_F32),
        (W64, Unsigned, F64, _) => Some(CONV_U64_TO_F64),

        // Float -> Integer
        (F32, _, W32, Signed) => Some(CONV_F32_TO_I32),
        (F32, _, W64, Signed) => Some(CONV_F32_TO_I64),
        (F64, _, W32, Signed) => Some(CONV_F64_TO_I32),
        (F64, _, W64, Signed) => Some(CONV_F64_TO_I64),
        (F32, _, W32, Unsigned) => Some(CONV_F32_TO_U32),
        (F32, _, W64, Unsigned) => Some(CONV_F32_TO_U64),
        (F64, _, W32, Unsigned) => Some(CONV_F64_TO_U32),
        (F64, _, W64, Unsigned) => Some(CONV_F64_TO_U64),

        // Float -> Float
        (F32, _, F64, _) => Some(CONV_F32_TO_F64),
        (F64, _, F32, _) => Some(CONV_F64_TO_F32),

        // Same float width (shouldn't happen, but handle gracefully)
        (F32, _, F32, _) | (F64, _, F64, _) => None,
    };
    match func_id {
        Some(func_id) => emitter.emit_builtin(func_id),
        None => Ok(()),
    }
}

// --- FB type helpers and string conversion (moved from compile.rs) ---

/// The VM block that runs the standard function block `block`, as
/// `(type_id, total_num_fields, field_name -> index)`.
///
/// The analyzer records which standard block a type is on the type itself,
/// so a user-defined block named like one never reaches this table.
pub(crate) fn standard_fb_layout(
    block: StandardFunctionBlock,
) -> (FbTypeId, usize, HashMap<String, u8>) {
    use CounterWidth::*;
    use StandardFunctionBlock::*;
    let (type_id, fields, field_map) = match block {
        Ton => (opcode::fb_type::TON, 6, timer_fb_fields()),
        Tof => (opcode::fb_type::TOF, 6, timer_fb_fields()),
        Tp => (opcode::fb_type::TP, 6, timer_fb_fields()),
        // Each width of a counter counts in its own type: `INT` and `DINT`
        // in a signed 32-bit integer, the others in their own.
        Ctu(Int | Dint) => (opcode::fb_type::CTU_DINT, 6, ctu_fb_fields()),
        Ctu(Udint) => (opcode::fb_type::CTU_UDINT, 6, ctu_fb_fields()),
        Ctu(Lint) => (opcode::fb_type::CTU_LINT, 6, ctu_fb_fields()),
        Ctu(Ulint) => (opcode::fb_type::CTU_ULINT, 6, ctu_fb_fields()),
        Ctd(Int | Dint) => (opcode::fb_type::CTD_DINT, 6, ctd_fb_fields()),
        Ctd(Udint) => (opcode::fb_type::CTD_UDINT, 6, ctd_fb_fields()),
        Ctd(Lint) => (opcode::fb_type::CTD_LINT, 6, ctd_fb_fields()),
        Ctd(Ulint) => (opcode::fb_type::CTD_ULINT, 6, ctd_fb_fields()),
        Ctud(Int | Dint) => (opcode::fb_type::CTUD_DINT, 10, ctud_fb_fields()),
        Ctud(Udint) => (opcode::fb_type::CTUD_UDINT, 10, ctud_fb_fields()),
        Ctud(Lint) => (opcode::fb_type::CTUD_LINT, 10, ctud_fb_fields()),
        Ctud(Ulint) => (opcode::fb_type::CTUD_ULINT, 10, ctud_fb_fields()),
        Sr => (opcode::fb_type::SR, 3, sr_fb_fields()),
        Rs => (opcode::fb_type::RS, 3, rs_fb_fields()),
        RTrig => (opcode::fb_type::R_TRIG, 3, edge_trig_fb_fields()),
        FTrig => (opcode::fb_type::F_TRIG, 3, edge_trig_fb_fields()),
    };
    (FbTypeId::new(type_id), fields, field_map)
}

/// The standard function block each type of `types` is, by the type's
/// upper-case name, as the type environment records it: the block whose VM
/// block [`standard_fb_layout`] gives a lowered call of a POU of that name.
pub(crate) fn standard_blocks_by_name(
    types: &TypeEnvironment,
) -> HashMap<String, StandardFunctionBlock> {
    types
        .iter()
        .filter_map(|(name, attributes)| {
            Some((name.to_string().to_uppercase(), attributes.standard_block?))
        })
        .collect()
}

/// Returns the shared field map for timer FBs (TON, TOF, TP).
/// Fields 4-5 are hidden (start_time, running) and not included.
fn timer_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("in".to_string(), 0);
    fields.insert("pt".to_string(), 1);
    fields.insert("q".to_string(), 2);
    fields.insert("et".to_string(), 3);
    fields
}

/// Returns the field map for CTU (count up) FBs.
/// Field 5 is hidden (prev_cu) and not included.
fn ctu_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTD (count down) FBs.
/// Field 5 is hidden (prev_cd) and not included.
fn ctd_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cd".to_string(), 0);
    fields.insert("ld".to_string(), 1);
    fields.insert("pv".to_string(), 2);
    fields.insert("q".to_string(), 3);
    fields.insert("cv".to_string(), 4);
    fields
}

/// Returns the field map for CTUD (count up/down) FBs.
/// Fields 8-9 are hidden (prev_cu, prev_cd) and not included.
fn ctud_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("cu".to_string(), 0);
    fields.insert("cd".to_string(), 1);
    fields.insert("r".to_string(), 2);
    fields.insert("ld".to_string(), 3);
    fields.insert("pv".to_string(), 4);
    fields.insert("qu".to_string(), 5);
    fields.insert("qd".to_string(), 6);
    fields.insert("cv".to_string(), 7);
    fields
}

/// Returns the field map for SR (set-reset) FBs.
fn sr_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s1".to_string(), 0);
    fields.insert("r".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for RS (reset-set) FBs.
fn rs_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("s".to_string(), 0);
    fields.insert("r1".to_string(), 1);
    fields.insert("q1".to_string(), 2);
    fields
}

/// Returns the field map for edge trigger FBs (R_TRIG, F_TRIG).
/// Field 2 is hidden (M / previous CLK) and not included.
fn edge_trig_fb_fields() -> HashMap<String, u8> {
    let mut fields = HashMap::new();
    fields.insert("clk".to_string(), 0);
    fields.insert("q".to_string(), 1);
    fields
}

/// Compiles a call to the conversion from `source` to `target`.
///
/// A conversion to or from `STRING` goes through the data region; any
/// other is a numeric conversion between operation types.
fn compile_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    span: &SourceSpan,
    source: &ElementaryTypeName,
    target: &ElementaryTypeName,
) -> Result<(), Diagnostic> {
    let type_info = |elementary: &ElementaryTypeName| {
        elementary_type_info(elementary).ok_or_else(|| Diagnostic::todo_with_span(span.clone()))
    };
    match (source, target) {
        (_, ElementaryTypeName::STRING) => {
            let source = type_info(source)?;
            compile_string_conversion(
                emitter,
                ctx,
                args,
                span,
                StringConversion::NumToString { source },
            )
        }
        (ElementaryTypeName::STRING, _) => {
            let target = type_info(target)?;
            compile_string_conversion(
                emitter,
                ctx,
                args,
                span,
                StringConversion::StringToNum { target },
            )
        }
        _ => {
            let (source, target) = (type_info(source)?, type_info(target)?);
            compile_type_conversion(emitter, ctx, args, span, source, target)
        }
    }
}

/// Describes a string ↔ numeric conversion direction.
enum StringConversion {
    /// Numeric → STRING (e.g., INT_TO_STRING, DWORD_TO_STRING).
    NumToString { source: VarTypeInfo },
    /// STRING → Numeric (e.g., STRING_TO_INT, STRING_TO_REAL).
    StringToNum { target: VarTypeInfo },
}

/// Compiles a string ↔ numeric conversion function call.
fn compile_string_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    args: [&Expr; 1],
    span: &SourceSpan,
    conv: StringConversion,
) -> Result<(), Diagnostic> {
    match conv {
        StringConversion::NumToString { source } => {
            compile_expr(emitter, ctx, args[0])?;

            let func_id = match (source.op_width, source.signedness) {
                (OpWidth::W32, Signedness::Signed) => opcode::builtin::CONV_I32_TO_STR,
                (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::CONV_U32_TO_STR,
                (OpWidth::F32, _) => opcode::builtin::CONV_F32_TO_STR,
                _ => {
                    return Err(Diagnostic::internal_error_at(Label::span(
                        span.clone(),
                        "Number-to-string conversion from a type with no conversion opcode",
                    )));
                }
            };
            emitter.emit_builtin(func_id)?;
            Ok(())
        }
        StringConversion::StringToNum { target } => {
            // STRING_TO_* parses Latin-1 digits, so a WSTRING argument has no
            // conversion. Analysis rejects one (P4026), so it is an internal
            // error rather than an encoding-mismatch trap.
            let data_offset = resolve_string_arg(emitter, ctx, args[0], span, NARROW_CHAR_WIDTH)?;
            let pool_index = ctx.add_i32_constant(data_offset as i32);
            emitter.emit_load_const_i32(pool_index);

            use opcode::builtin::str_to_num::{func_id as block_func_id, Target};
            // Every target is a policy-bearing conversion (ADR-0049): the
            // func_id names the target and both selected policies, the VM
            // range-checks against the target's own bounds, and no
            // truncation follows. `STRING_TO_SINT('300')` fails; it never
            // wraps to 44. A bit-string type has the signedness and value
            // width of the unsigned integer it aliases, so it lands on that
            // integer's target.
            let block = |target: Target| {
                block_func_id(
                    target,
                    ctx.string_to_num.non_numeric,
                    ctx.string_to_num.failure,
                )
            };
            let func_id = match (target.op_width, target.signedness, target.storage_bits) {
                (OpWidth::W32, Signedness::Unsigned, 32) => block(Target::U32),
                (OpWidth::W32, Signedness::Signed, 32) => block(Target::I32),
                (OpWidth::W32, Signedness::Unsigned, 8) => block(Target::U8),
                (OpWidth::W32, Signedness::Signed, 8) => block(Target::I8),
                (OpWidth::W32, Signedness::Unsigned, 16) => block(Target::U16),
                (OpWidth::W32, Signedness::Signed, 16) => block(Target::I16),
                (OpWidth::W64, Signedness::Unsigned, 64) => block(Target::U64),
                (OpWidth::W64, Signedness::Signed, 64) => block(Target::I64),
                (OpWidth::F32, _, _) => block(Target::F32),
                (OpWidth::F64, _, _) => block(Target::F64),
                // A 64-bit slot holds only the 64-bit value width.
                (OpWidth::W64, _, _) => {
                    return Err(Diagnostic::internal_error_at(Label::span(
                        span.clone(),
                        "STRING_TO_* 64-bit target has no conversion",
                    )));
                }
                // A 32-bit slot holds only the value widths above. The
                // analyzer offers no other STRING_TO_* with a 32-bit
                // target (there is no STRING_TO_BOOL), so this is a
                // compiler bug, not a program error.
                (OpWidth::W32, _, _) => {
                    return Err(Diagnostic::internal_error_at(Label::span(
                        span.clone(),
                        "STRING_TO_* target has no conversion",
                    )));
                }
            };
            emitter.emit_builtin(func_id)?;
            Ok(())
        }
    }
}
