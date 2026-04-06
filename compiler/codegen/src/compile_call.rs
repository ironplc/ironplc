//! Function call compilation for IEC 61131-3 code generation.
//!
//! Contains standard library function dispatch, user-defined function calls,
//! builtin lookup, type conversions, time functions, and shift/rotate operations.
//! Separated from compile.rs to keep module sizes within the 1000-line guideline.

use std::collections::HashMap;

use ironplc_container::opcode;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{
    Expr, ExprKind, Function, ParamAssignmentKind, SymbolicVariableKind, Variable,
};

use super::compile::{
    CompileContext, OpType, OpWidth, Signedness, UserFunctionInfo, VarTypeInfo, DEFAULT_OP_TYPE,
};
use super::compile_expr::{
    compile_expr, emit_add, emit_and, emit_div, emit_eq, emit_ge, emit_gt, emit_le, emit_lt,
    emit_mod, emit_mul, emit_ne, emit_or, emit_sub, emit_truncation, emit_xor, op_type,
    op_type_from_expr, storage_bits,
};
use super::compile_setup::resolve_type_name;
use super::compile_string::{
    compile_concat, compile_delete, compile_find, compile_insert, compile_left, compile_len,
    compile_mid, compile_replace, compile_right, resolve_string_arg,
};
use crate::emit::Emitter;

/// Returns the builtin opcode for a named standard library function, if known.
///
/// The `op_width` selects the correct width variant and `signedness` selects
/// the signed/unsigned variant for functions that distinguish them.
pub(crate) fn lookup_builtin(name: &str, op_width: OpWidth, signedness: Signedness) -> Option<u16> {
    match name.to_uppercase().as_str() {
        "EXPT" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::EXPT_I32,
            OpWidth::W64 => opcode::builtin::EXPT_I64,
            OpWidth::F32 => opcode::builtin::EXPT_F32,
            OpWidth::F64 => opcode::builtin::EXPT_F64,
        }),
        "ABS" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::ABS_I32,
            OpWidth::W64 => opcode::builtin::ABS_I64,
            OpWidth::F32 => opcode::builtin::ABS_F32,
            OpWidth::F64 => opcode::builtin::ABS_F64,
        }),
        "MIN" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MIN_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MIN_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MIN_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MIN_U64,
            (OpWidth::F32, _) => opcode::builtin::MIN_F32,
            (OpWidth::F64, _) => opcode::builtin::MIN_F64,
        }),
        "MAX" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MAX_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MAX_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MAX_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MAX_U64,
            (OpWidth::F32, _) => opcode::builtin::MAX_F32,
            (OpWidth::F64, _) => opcode::builtin::MAX_F64,
        }),
        "LIMIT" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::LIMIT_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::LIMIT_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::LIMIT_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::LIMIT_U64,
            (OpWidth::F32, _) => opcode::builtin::LIMIT_F32,
            (OpWidth::F64, _) => opcode::builtin::LIMIT_F64,
        }),
        "SEL" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::SEL_I32,
            OpWidth::W64 => opcode::builtin::SEL_I64,
            OpWidth::F32 => opcode::builtin::SEL_F32,
            OpWidth::F64 => opcode::builtin::SEL_F64,
        }),
        "SQRT" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SQRT_F32),
            OpWidth::F64 => Some(opcode::builtin::SQRT_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "LN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LN_F32),
            OpWidth::F64 => Some(opcode::builtin::LN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "LOG" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LOG_F32),
            OpWidth::F64 => Some(opcode::builtin::LOG_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "EXP" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::EXP_F32),
            OpWidth::F64 => Some(opcode::builtin::EXP_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "SIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SIN_F32),
            OpWidth::F64 => Some(opcode::builtin::SIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "COS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::COS_F32),
            OpWidth::F64 => Some(opcode::builtin::COS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "TAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::TAN_F32),
            OpWidth::F64 => Some(opcode::builtin::TAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ASIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ASIN_F32),
            OpWidth::F64 => Some(opcode::builtin::ASIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ACOS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ACOS_F32),
            OpWidth::F64 => Some(opcode::builtin::ACOS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ATAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ATAN_F32),
            OpWidth::F64 => Some(opcode::builtin::ATAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ATAN2" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ATAN2_F32),
            OpWidth::F64 => Some(opcode::builtin::ATAN2_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        _ => None,
    }
}

/// Compiles a standard library function call.
///
/// Dispatches shift/rotate functions to a width-aware handler, and other
/// known builtins to the generic lookup path.
pub(crate) fn compile_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "shl" | "shr" | "rol" | "ror" => {
            compile_shift_rotate(emitter, ctx, func, op_type, name.as_str())
        }
        "mux" => compile_mux(emitter, ctx, func, op_type),
        // Arithmetic function forms (equivalent to +, -, *, /, MOD operators)
        "add" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_add),
        "sub" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_sub),
        "mul" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_mul),
        "div" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_div),
        "mod" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_mod),
        // Comparison function forms (equivalent to >, >=, =, <=, <, <> operators)
        "gt" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_gt),
        "ge" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_ge),
        "eq" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_eq),
        "le" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_le),
        "lt" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_lt),
        "ne" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_ne),
        // Boolean function forms (equivalent to AND, OR, XOR, NOT operators)
        "and" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_and),
        "or" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_or),
        "xor" => compile_two_arg_operator(emitter, ctx, func, op_type, emit_xor),
        "not" => compile_not_function(emitter, ctx, func, op_type),
        // Assignment function (equivalent to := operator)
        "move" => compile_move(emitter, ctx, func, op_type),
        // Truncation function
        "trunc" => compile_trunc(emitter, ctx, func, op_type),
        // SIZEOF operator (vendor extension)
        "sizeof" => compile_sizeof(emitter, ctx, func),
        // BCD conversion functions
        "bcd_to_int" => compile_bcd_to_int(emitter, ctx, func, op_type),
        "int_to_bcd" => compile_int_to_bcd(emitter, ctx, func, op_type),
        // String functions
        "len" => compile_len(emitter, ctx, func),
        "find" => compile_find(emitter, ctx, func),
        "replace" => compile_replace(emitter, ctx, func),
        "insert" => compile_insert(emitter, ctx, func),
        "delete" => compile_delete(emitter, ctx, func),
        "left" => compile_left(emitter, ctx, func),
        "right" => compile_right(emitter, ctx, func),
        "mid" => compile_mid(emitter, ctx, func),
        "concat" => compile_concat(emitter, ctx, func),
        // Time functions — Group 1: direct i32 operations (same units)
        "add_time" | "add_tod_time" => compile_two_arg_operator(
            emitter,
            ctx,
            func,
            (OpWidth::W32, Signedness::Signed),
            emit_add,
        ),
        "sub_time" | "sub_tod_time" | "sub_tod_tod" => compile_two_arg_operator(
            emitter,
            ctx,
            func,
            (OpWidth::W32, Signedness::Signed),
            emit_sub,
        ),
        // Time functions — Group 2: ms-to-seconds conversion before add/sub
        "add_dt_time" | "concat_date_tod" => compile_dt_time_add_sub(emitter, ctx, func, emit_add),
        "sub_dt_time" => compile_dt_time_add_sub(emitter, ctx, func, emit_sub),
        // Time functions — Group 3: seconds-to-ms conversion after sub
        "sub_dt_dt" | "sub_date_date" => compile_sub_to_time(emitter, ctx, func),
        // Time functions — Group 5: datetime decomposition
        "dt_to_date" | "date_and_time_to_date" => compile_dt_to_date(emitter, ctx, func),
        "dt_to_tod" | "date_and_time_to_time_of_day" => compile_dt_to_tod(emitter, ctx, func),
        // Time functions — Group 4: type-dependent MUL/DIV
        "mul_time" => compile_mul_div_time(emitter, ctx, func, true),
        "div_time" => compile_mul_div_time(emitter, ctx, func, false),
        _ => {
            // Check user-defined functions first.
            if let Some(func_info) = ctx.user_functions.get(name.as_str()).cloned() {
                compile_user_function_call(emitter, ctx, func, &func_info)
            } else if let Some(conv) = parse_string_conversion(name) {
                compile_string_conversion(emitter, ctx, func, conv)
            } else if let Some((source, target)) = parse_type_conversion(name) {
                compile_type_conversion(emitter, ctx, func, source, target)
            } else {
                compile_generic_builtin(emitter, ctx, func, op_type)
            }
        }
    }
}

/// Compiles a call to a user-defined function.
///
/// For STRING parameters, copies the caller's string data into the function's
/// pre-allocated data region space before the CALL. For scalar parameters,
/// compiles the argument expression normally. The CALL opcode pops scalar
/// arguments (and dummy values for STRING params) from the stack, stores them
/// into the function's variable slots, executes the function, and pushes
/// the return value onto the stack.
fn compile_user_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    func_info: &UserFunctionInfo,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    // Compile each argument with the corresponding parameter's OpType.
    // STRING parameters are copied into the function's data region before CALL;
    // a dummy zero is pushed for the stack pop count.
    for (i, arg) in args.iter().enumerate() {
        if let Some(Some(str_info)) = func_info.param_string_info.get(i) {
            // Copy the string argument into the function's parameter space.
            // Initialize the destination header, then copy the string data.
            emitter.emit_str_init(str_info.data_offset, str_info.max_length);
            let src_offset = resolve_string_arg(emitter, ctx, arg, &func.name.span())?;
            ctx.num_temp_bufs += 1;
            emitter.emit_str_load_var(src_offset);
            emitter.emit_str_store_var(str_info.data_offset);

            // Push a dummy value for the CALL stack pop.
            let zero_idx = ctx.add_i32_constant(0);
            emitter.emit_load_const_i32(zero_idx);
        } else {
            let param_op_type = func_info
                .param_op_types
                .get(i)
                .copied()
                .unwrap_or(DEFAULT_OP_TYPE);

            // When implicit integer widening crosses OpWidth boundaries
            // (e.g. INT [W32] -> LINT [W64]), compile the argument at its
            // natural width and then emit a conversion opcode.
            let arg_natural = arg
                .resolved_type
                .as_ref()
                .and_then(|t| resolve_type_name(&t.name))
                .map(|info| (info.op_width, info.signedness));

            if let Some(arg_op) = arg_natural {
                if arg_op.0 != param_op_type.0 {
                    compile_expr(emitter, ctx, arg, arg_op)?;
                    let source = VarTypeInfo {
                        op_width: arg_op.0,
                        signedness: arg_op.1,
                        storage_bits: 0,
                    };
                    let target = VarTypeInfo {
                        op_width: param_op_type.0,
                        signedness: param_op_type.1,
                        storage_bits: 0,
                    };
                    emit_conversion_opcode(emitter, &source, &target);
                } else {
                    compile_expr(emitter, ctx, arg, param_op_type)?;
                }
            } else {
                compile_expr(emitter, ctx, arg, param_op_type)?;
            }
        }
    }

    // If the function returns STRING, initialize the return string's header
    // in the data region before CALL so the function body can write to it.
    if let Some(ref ret_str) = func_info.return_string_info {
        emitter.emit_str_init(ret_str.data_offset, ret_str.max_length);
    }

    emitter.emit_call(
        func_info.function_id,
        func_info.num_params,
        func_info.var_offset,
        func_info.max_stack_depth,
    );
    // For STRING-returning functions, the CALL leaves a buf_idx on the stack
    // (from emit_str_load_var in the function epilogue). The caller's
    // assignment path will consume it via emit_str_store_var.
    Ok(())
}

/// Compiles a generic builtin function call via `lookup_builtin`.
///
/// All arguments are compiled with the same `op_type` and the function ID
/// is looked up by name.
fn compile_generic_builtin(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let func_name = func.name.original().to_uppercase();
    let func_id = lookup_builtin(&func_name, op_type.0, op_type.1)
        .ok_or_else(|| Diagnostic::todo_with_span(func.name.span(), file!(), line!()))?;

    let expected_args = opcode::builtin::arg_count(func_id) as usize;

    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != expected_args {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let is_sel = func_name == "SEL";
    for (i, arg) in args.iter().enumerate() {
        // SEL's first argument (G) is always a BOOL/integer selector,
        // even when the remaining arguments are float.
        let arg_op_type = if is_sel && i == 0 {
            DEFAULT_OP_TYPE
        } else {
            op_type
        };
        compile_expr(emitter, ctx, arg, arg_op_type)?;
    }

    emitter.emit_builtin(func_id);
    Ok(())
}

/// Compiles a two-argument function form that maps to an existing operator.
///
/// Extracts the two positional arguments, compiles them with the given `op_type`,
/// and calls the provided emit function. This is used for function forms like
/// ADD(a, b) which are equivalent to the operator form a + b.
fn compile_two_arg_operator(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], op_type)?;
    compile_expr(emitter, ctx, args[1], op_type)?;
    emit_fn(emitter, op_type);
    Ok(())
}

/// Extracts two positional input arguments from a function call.
fn extract_two_positional_args(func: &Function) -> Result<(&Expr, &Expr), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    Ok((args[0], args[1]))
}

/// Compiles ADD_DT_TIME, SUB_DT_TIME, and CONCAT_DATE_TOD.
///
/// IN2 (TIME or TOD) is in milliseconds while IN1 (DT or DATE) is in seconds.
/// Converts IN2 from ms to seconds by dividing by 1000, then adds or subtracts.
fn compile_dt_time_add_sub(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let (in1, in2) = extract_two_positional_args(func)?;
    let op_type = (OpWidth::W32, Signedness::Signed);
    compile_expr(emitter, ctx, in1, op_type)?;
    compile_expr(emitter, ctx, in2, op_type)?;
    let pool_idx = ctx.add_i32_constant(1000);
    emitter.emit_load_const_i32(pool_idx);
    emit_div(emitter, op_type);
    emit_fn(emitter, op_type);
    Ok(())
}

/// Compiles SUB_DT_DT and SUB_DATE_DATE.
///
/// Subtracts two values in seconds, then multiplies by 1000 to produce TIME
/// in milliseconds.
fn compile_sub_to_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let (in1, in2) = extract_two_positional_args(func)?;
    let op_type = (OpWidth::W32, Signedness::Signed);
    compile_expr(emitter, ctx, in1, op_type)?;
    compile_expr(emitter, ctx, in2, op_type)?;
    emit_sub(emitter, op_type);
    let pool_idx = ctx.add_i32_constant(1000);
    emitter.emit_load_const_i32(pool_idx);
    emit_mul(emitter, op_type);
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
    func: &Function,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let op_type = (OpWidth::W32, Signedness::Unsigned);
    // Stack: IN
    compile_expr(emitter, ctx, args[0], op_type)?;
    // Stack: IN, IN
    compile_expr(emitter, ctx, args[0], op_type)?;
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
    func: &Function,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let op_type = (OpWidth::W32, Signedness::Unsigned);
    // Stack: IN
    compile_expr(emitter, ctx, args[0], op_type)?;
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

/// Compiles MUL_TIME and DIV_TIME.
///
/// IN1 is TIME (i32 ms). IN2 is ANY_NUM — codegen inspects IN2's resolved type
/// to select the appropriate instruction sequence. For integer IN2 we use
/// direct i32 multiply/divide. For float IN2 we convert TIME to float, operate,
/// and convert back.
fn compile_mul_div_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    is_mul: bool,
) -> Result<(), Diagnostic> {
    let (in1, in2) = extract_two_positional_args(func)?;
    let time_op = (OpWidth::W32, Signedness::Signed);

    let in2_op = op_type_from_expr(in2).unwrap_or(time_op);

    match in2_op.0 {
        OpWidth::W32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            compile_expr(emitter, ctx, in2, time_op)?;
            if is_mul {
                emit_mul(emitter, time_op);
            } else {
                emit_div(emitter, time_op);
            }
        }
        OpWidth::F32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32);
            compile_expr(emitter, ctx, in2, (OpWidth::F32, Signedness::Signed))?;
            let f32_op = (OpWidth::F32, Signedness::Signed);
            if is_mul {
                emit_mul(emitter, f32_op);
            } else {
                emit_div(emitter, f32_op);
            }
            emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32);
        }
        OpWidth::F64 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64);
            compile_expr(emitter, ctx, in2, (OpWidth::F64, Signedness::Signed))?;
            let f64_op = (OpWidth::F64, Signedness::Signed);
            if is_mul {
                emit_mul(emitter, f64_op);
            } else {
                emit_div(emitter, f64_op);
            }
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32);
        }
        OpWidth::W64 => {
            // LINT/ULINT: promote TIME to f64, convert IN2 to f64, operate, convert back.
            // This avoids needing an i64→i32 truncation opcode.
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64);
            compile_expr(emitter, ctx, in2, (OpWidth::W64, in2_op.1))?;
            emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64);
            let f64_op = (OpWidth::F64, Signedness::Signed);
            if is_mul {
                emit_mul(emitter, f64_op);
            } else {
                emit_div(emitter, f64_op);
            }
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32);
        }
    }

    Ok(())
}

/// Compiles the NOT function form.
///
/// NOT(IN) is equivalent to the NOT operator. Takes a single BOOL argument.
fn compile_not_function(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], op_type)?;
    emitter.emit_bool_not();
    Ok(())
}

/// Compiles the MOVE function form.
///
/// MOVE(IN) is equivalent to assignment. Takes a single argument and returns
/// it unchanged. No opcode is needed since the value is already on the stack.
fn compile_move(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], op_type)?;
    // No additional opcode needed - the value is already on the stack

    Ok(())
}

/// Compiles TRUNC(IN) — truncates a real value toward zero.
///
/// The argument is compiled using its own (float) op_type derived from the
/// argument's resolved type. The result is converted to the target integer
/// type using the existing conversion opcodes.
fn compile_trunc(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    target_op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    // Determine the argument's float type from its resolved type.
    let arg_op_type = op_type(args[0])?;
    compile_expr(emitter, ctx, args[0], arg_op_type)?;

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
    emit_conversion_opcode(emitter, &source, &target);

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
    func: &Function,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    // Check if the argument is a variable that maps to an array.
    let size: u32 =
        if let ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(ref named))) =
            args[0].kind
        {
            if let Some(array_info) = ctx.array_vars.get(&named.name) {
                let elem_bytes = array_info.element_var_type_info.storage_bits as u32 / 8;
                array_info.total_elements * elem_bytes
            } else {
                sizeof_from_resolved_type(args[0])?
            }
        } else {
            sizeof_from_resolved_type(args[0])?
        };

    let pool_index = ctx.add_i32_constant(size as i32);
    emitter.emit_load_const_i32(pool_index);
    Ok(())
}

/// Returns the size in bytes from an expression's resolved type annotation.
fn sizeof_from_resolved_type(expr: &Expr) -> Result<u32, Diagnostic> {
    let resolved = expr
        .resolved_type
        .as_ref()
        .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    let info =
        resolve_type_name(&resolved.name).ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    // Ceiling division: types like BOOL (1 bit) still occupy 1 byte.
    Ok((info.storage_bits as u32).div_ceil(8))
}

/// Compiles BCD_TO_INT(IN) — converts a BCD-encoded bit string to an integer.
///
/// The argument is compiled using its own (bit-string) op_type. The BCD
/// decoding opcode is selected based on the argument's storage bit width.
fn compile_bcd_to_int(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    _target_op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let arg_op_type = op_type(args[0])?;
    let bits = storage_bits(args[0])?;
    compile_expr(emitter, ctx, args[0], arg_op_type)?;

    let func_id = match bits {
        8 => opcode::builtin::BCD_TO_INT_8,
        16 => opcode::builtin::BCD_TO_INT_16,
        32 => opcode::builtin::BCD_TO_INT_32,
        64 => opcode::builtin::BCD_TO_INT_64,
        _ => {
            return Err(Diagnostic::todo_with_span(
                func.name.span(),
                file!(),
                line!(),
            ))
        }
    };
    emitter.emit_builtin(func_id);
    Ok(())
}

/// Compiles INT_TO_BCD(IN) — converts an integer to a BCD-encoded bit string.
///
/// The argument is compiled using its own (integer) op_type. The BCD
/// encoding opcode is selected based on the target's operation width.
fn compile_int_to_bcd(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    target_op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let arg_op_type = op_type(args[0])?;
    let bits = storage_bits(args[0])?;
    compile_expr(emitter, ctx, args[0], arg_op_type)?;

    let func_id = match (arg_op_type.0, bits) {
        (OpWidth::W32, 8) => opcode::builtin::INT_TO_BCD_8,
        (OpWidth::W32, 16) => opcode::builtin::INT_TO_BCD_16,
        (OpWidth::W32, 32) => opcode::builtin::INT_TO_BCD_32,
        (OpWidth::W64, 64) => opcode::builtin::INT_TO_BCD_64,
        _ => {
            // For wider target than source, select based on target width
            match target_op_type.0 {
                OpWidth::W32 => opcode::builtin::INT_TO_BCD_32,
                OpWidth::W64 => opcode::builtin::INT_TO_BCD_64,
                _ => {
                    return Err(Diagnostic::todo_with_span(
                        func.name.span(),
                        file!(),
                        line!(),
                    ))
                }
            }
        }
    };
    emitter.emit_builtin(func_id);
    Ok(())
}

/// Compiles a MUX (multiplexer) function call.
///
/// MUX(K, IN0, IN1, ..., INn) selects one of the IN values based on the
/// integer selector K. The first argument K is always compiled as I32
/// (integer selector), while the remaining IN arguments use the caller's op_type.
///
/// The opcode encodes the number of IN arguments: `MUX_<WIDTH>_BASE + num_inputs`.
fn compile_mux(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    // Must have at least 3 args (K + 2 IN values)
    if args.len() < 3 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let num_inputs = (args.len() - 1) as u16; // subtract K

    if num_inputs > opcode::builtin::MUX_MAX_INPUTS {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let base = match op_type.0 {
        OpWidth::W32 => opcode::builtin::MUX_I32_BASE,
        OpWidth::W64 => opcode::builtin::MUX_I64_BASE,
        OpWidth::F32 => opcode::builtin::MUX_F32_BASE,
        OpWidth::F64 => opcode::builtin::MUX_F64_BASE,
    };
    let func_id = base + num_inputs;

    // Compile K (first arg) as integer
    compile_expr(emitter, ctx, args[0], DEFAULT_OP_TYPE)?;

    // Compile IN0..INn with the caller's op_type
    for arg in &args[1..] {
        compile_expr(emitter, ctx, arg, op_type)?;
    }

    emitter.emit_builtin(func_id);
    Ok(())
}

/// Collects positional input arguments from a function call.
pub(crate) fn collect_positional_args(func: &Function) -> Vec<&Expr> {
    func.param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect()
}

/// Compiles a type conversion function call (e.g., INT_TO_REAL).
///
/// Unlike generic builtins, conversion functions have different source and
/// target types. The argument is compiled with the source type's OpType,
/// then a conversion opcode (if needed) transforms the value to the target
/// representation.
pub(crate) fn compile_type_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    source: VarTypeInfo,
    target: VarTypeInfo,
) -> Result<(), Diagnostic> {
    let source_op_type: OpType = (source.op_width, source.signedness);

    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], source_op_type)?;

    // Integer-to-boolean needs a dedicated opcode (non-zero → 1, zero → 0)
    // rather than the generic conversion + truncation path, because
    // truncation would only keep the lowest bit instead of testing for zero.
    if target.storage_bits == 1 {
        match source.op_width {
            OpWidth::W32 => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_BOOL),
            OpWidth::W64 => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_BOOL),
            _ => {
                return Err(Diagnostic::todo_with_span(
                    func.name.span(),
                    file!(),
                    line!(),
                ));
            }
        }
    } else {
        emit_conversion_opcode(emitter, &source, &target);
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
) {
    use OpWidth::*;
    use Signedness::*;

    match (
        source.op_width,
        source.signedness,
        target.op_width,
        target.signedness,
    ) {
        // Same OpWidth: no conversion needed (truncation handles sub-width)
        (W32, _, W32, _) | (W64, _, W64, _) => {}

        // W32 signed -> W64: sign extension already in Slot, no-op
        (W32, Signed, W64, _) => {}

        // W32 unsigned -> W64: need zero-extension
        (W32, Unsigned, W64, _) => {
            emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        }

        // W64 -> W32: as_i32() truncation at store time, no-op
        (W64, _, W32, _) => {}

        // Integer -> Float
        (W32, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32),
        (W32, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64),
        (W64, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F32),
        (W64, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64),
        (W32, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F32),
        (W32, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F64),
        (W64, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F32),
        (W64, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F64),

        // Float -> Integer
        (F32, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32),
        (F32, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I64),
        (F64, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32),
        (F64, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I64),
        (F32, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U32),
        (F32, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U64),
        (F64, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U32),
        (F64, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U64),

        // Float -> Float
        (F32, _, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_F64),
        (F64, _, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_F32),

        // Same float width (shouldn't happen, but handle gracefully)
        (F32, _, F32, _) | (F64, _, F64, _) => {}
    }
}

/// Compiles a bit shift or rotate function call (SHL, SHR, ROL, ROR).
///
/// Expects two positional arguments: IN (value) and N (shift count).
/// Emits the appropriate BUILTIN opcode based on function name and operand width.
/// For ROL/ROR on narrow types (BYTE, WORD), emits width-specific builtins
/// to ensure bits wrap correctly within the narrow type.
fn compile_shift_rotate(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
    name: &str,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    // Compile IN (value) with the inferred op_type
    compile_expr(emitter, ctx, args[0], op_type)?;
    // Compile N (shift count) — always as i32 for W32, i64 for W64
    let n_op_type = match op_type.0 {
        OpWidth::W64 => (OpWidth::W64, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    };
    compile_expr(emitter, ctx, args[1], n_op_type)?;

    // Determine storage bits for narrow-type ROL/ROR selection
    let bits = storage_bits(args[0])?;

    let func_id = match (name, op_type.0) {
        ("shl", OpWidth::W64) => opcode::builtin::SHL_I64,
        ("shl", _) => opcode::builtin::SHL_I32,
        ("shr", OpWidth::W64) => opcode::builtin::SHR_I64,
        ("shr", _) => opcode::builtin::SHR_I32,
        ("rol", OpWidth::W64) => opcode::builtin::ROL_I64,
        ("rol", _) => match bits {
            8 => opcode::builtin::ROL_U8,
            16 => opcode::builtin::ROL_U16,
            _ => opcode::builtin::ROL_I32,
        },
        ("ror", OpWidth::W64) => opcode::builtin::ROR_I64,
        ("ror", _) => match bits {
            8 => opcode::builtin::ROR_U8,
            16 => opcode::builtin::ROR_U16,
            _ => opcode::builtin::ROR_I32,
        },
        _ => {
            return Err(Diagnostic::todo_with_span(
                func.name.span(),
                file!(),
                line!(),
            ))
        }
    };

    emitter.emit_builtin(func_id);
    Ok(())
}

// --- FB type helpers and string conversion (moved from compile.rs) ---

/// Resolves a standard FB type name to its (type_id, total_num_fields, field_name->index map).
/// Returns None for unknown FB types.
pub(crate) fn resolve_fb_type(name: &str) -> Option<(u16, usize, HashMap<String, u8>)> {
    match name {
        "TON" => Some((opcode::fb_type::TON, 6, timer_fb_fields())),
        "TOF" => Some((opcode::fb_type::TOF, 6, timer_fb_fields())),
        "TP" => Some((opcode::fb_type::TP, 6, timer_fb_fields())),
        "CTU" | "CTU_INT" | "CTU_DINT" | "CTU_LINT" | "CTU_UDINT" | "CTU_ULINT" => {
            Some((opcode::fb_type::CTU, 6, ctu_fb_fields()))
        }
        "CTD" | "CTD_INT" | "CTD_DINT" | "CTD_LINT" | "CTD_UDINT" | "CTD_ULINT" => {
            Some((opcode::fb_type::CTD, 6, ctd_fb_fields()))
        }
        "CTUD" | "CTUD_INT" | "CTUD_DINT" | "CTUD_LINT" | "CTUD_UDINT" | "CTUD_ULINT" => {
            Some((opcode::fb_type::CTUD, 10, ctud_fb_fields()))
        }
        "SR" => Some((opcode::fb_type::SR, 3, sr_fb_fields())),
        "RS" => Some((opcode::fb_type::RS, 3, rs_fb_fields())),
        "R_TRIG" => Some((opcode::fb_type::R_TRIG, 3, edge_trig_fb_fields())),
        "F_TRIG" => Some((opcode::fb_type::F_TRIG, 3, edge_trig_fb_fields())),
        _ => None,
    }
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

/// Checks if a function name is a type conversion (e.g., "int_to_real").
pub(crate) fn parse_type_conversion(name: &str) -> Option<(VarTypeInfo, VarTypeInfo)> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    let source = resolve_type_name(&Id::from(parts[0]))?;
    let target = resolve_type_name(&Id::from(parts[1]))?;
    Some((source, target))
}

/// Describes a string ↔ numeric conversion direction.
pub(crate) enum StringConversion {
    /// Numeric → STRING (e.g., INT_TO_STRING, DWORD_TO_STRING).
    NumToString { source: VarTypeInfo },
    /// STRING → Numeric (e.g., STRING_TO_INT, STRING_TO_REAL).
    StringToNum { target: VarTypeInfo },
}

/// Checks if a function name is a string conversion (e.g., "int_to_string").
///
/// Returns `Some(StringConversion)` if the name matches `*_TO_STRING` or
/// `STRING_TO_*` and the non-string part is a recognized type name.
pub(crate) fn parse_string_conversion(name: &str) -> Option<StringConversion> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    if parts[1] == "STRING" {
        let source = resolve_type_name(&Id::from(parts[0]))?;
        Some(StringConversion::NumToString { source })
    } else if parts[0] == "STRING" {
        let target = resolve_type_name(&Id::from(parts[1]))?;
        Some(StringConversion::StringToNum { target })
    } else {
        None
    }
}

/// Compiles a string ↔ numeric conversion function call.
pub(crate) fn compile_string_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    conv: StringConversion,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);
    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    match conv {
        StringConversion::NumToString { source } => {
            let source_op_type: OpType = (source.op_width, source.signedness);
            compile_expr(emitter, ctx, args[0], source_op_type)?;

            let func_id = match (source.op_width, source.signedness) {
                (OpWidth::W32, Signedness::Signed) => opcode::builtin::CONV_I32_TO_STR,
                (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::CONV_U32_TO_STR,
                (OpWidth::F32, _) => opcode::builtin::CONV_F32_TO_STR,
                _ => {
                    return Err(Diagnostic::todo_with_span(
                        func.name.span(),
                        file!(),
                        line!(),
                    ));
                }
            };
            emitter.emit_builtin(func_id);
            ctx.num_temp_bufs += 1;
            Ok(())
        }
        StringConversion::StringToNum { target } => {
            let data_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
            let pool_index = ctx.add_i32_constant(data_offset as i32);
            emitter.emit_load_const_i32(pool_index);

            let func_id = match target.op_width {
                OpWidth::W32 => opcode::builtin::CONV_STR_TO_I32,
                OpWidth::F32 => opcode::builtin::CONV_STR_TO_F32,
                _ => {
                    return Err(Diagnostic::todo_with_span(
                        func.name.span(),
                        file!(),
                        line!(),
                    ));
                }
            };
            emitter.emit_builtin(func_id);
            emit_truncation(emitter, target);
            Ok(())
        }
    }
}
