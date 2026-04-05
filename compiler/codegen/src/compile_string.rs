//! String function compilation for IEC 61131-3 code generation.
//!
//! Contains compilation of standard string functions (LEN, FIND, REPLACE,
//! INSERT, DELETE, LEFT, RIGHT, MID, CONCAT) and string comparison.
//! Separated from compile.rs to keep module sizes within the 1000-line guideline.

use ironplc_container::{opcode, STRING_HEADER_BYTES};
use ironplc_dsl::common::ConstantKind;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{CompareExpr, CompareOp, Expr, ExprKind, Function, ParamAssignmentKind};

use super::compile::{CompileContext, DEFAULT_OP_TYPE, DEFAULT_STRING_MAX_LENGTH_U16};
use super::compile_expr::{compile_expr, resolve_variable_name};
use crate::emit::Emitter;

/// Compiles the LEN standard function call.
///
/// LEN takes a single STRING variable argument and returns its current length
/// as an i32. Instead of going through the BUILTIN dispatch, LEN uses the
/// dedicated `LEN_STR` opcode which reads `cur_length` directly from the
/// string's data region header.
pub(crate) fn compile_len(
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

    // The argument must be a string variable so we can look up its data_offset.
    let var_name = match &args[0].kind {
        ExprKind::Variable(variable) => resolve_variable_name(variable),
        _ => None,
    };

    let name =
        var_name.ok_or_else(|| Diagnostic::todo_with_span(func.name.span(), file!(), line!()))?;

    let info = ctx
        .string_vars
        .get(name)
        .ok_or_else(|| Diagnostic::todo_with_span(func.name.span(), file!(), line!()))?;

    emitter.emit_len_str(info.data_offset);
    Ok(())
}

/// Compiles a string comparison expression.
///
/// Emits a `CMP_STR` builtin call (three-way comparison returning -1/0/+1),
/// followed by an integer comparison against zero to produce the boolean result.
pub(crate) fn compile_string_compare(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    compare: &CompareExpr,
) -> Result<(), Diagnostic> {
    let span = SourceSpan::default();
    let left_offset = resolve_string_arg(emitter, ctx, &compare.left, &span)?;
    let right_offset = resolve_string_arg(emitter, ctx, &compare.right, &span)?;

    // Push data_offsets as stack values.
    let left_pool = ctx.add_i32_constant(left_offset as i32);
    emitter.emit_load_const_i32(left_pool);
    let right_pool = ctx.add_i32_constant(right_offset as i32);
    emitter.emit_load_const_i32(right_pool);

    emitter.emit_builtin(opcode::builtin::CMP_STR);

    let zero_idx = ctx.add_i32_constant(0);
    emitter.emit_load_const_i32(zero_idx);

    match compare.op {
        CompareOp::Eq => emitter.emit_eq_i32(),
        CompareOp::Ne => emitter.emit_ne_i32(),
        CompareOp::Lt => emitter.emit_lt_i32(),
        CompareOp::Gt => emitter.emit_gt_i32(),
        CompareOp::LtEq => emitter.emit_le_i32(),
        CompareOp::GtEq => emitter.emit_ge_i32(),
        _ => {
            return Err(Diagnostic::todo_with_span(span, file!(), line!()));
        }
    }
    Ok(())
}

/// Resolves a string argument to its data_offset in the data region.
///
/// Handles both variable references (looked up in `string_vars`) and
/// string literals (allocated inline in the data region with initialization
/// code emitted at the point of use).
pub(crate) fn resolve_string_arg(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arg: &Expr,
    func_span: &ironplc_dsl::core::SourceSpan,
) -> Result<u32, Diagnostic> {
    match &arg.kind {
        ExprKind::Variable(variable) => {
            // Fast path: simple named variable found in string_vars.
            if let Some(var_name) = resolve_variable_name(variable) {
                if let Some(info) = ctx.string_vars.get(var_name) {
                    return Ok(info.data_offset);
                }
            }
            // Complex variable (e.g., struct field array subscript): fall
            // through to the general expression path below.
            let max_length = DEFAULT_STRING_MAX_LENGTH_U16;
            let data_offset = ctx.data_region_offset;
            let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
            ctx.data_region_offset = ctx
                .data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| Diagnostic::todo_with_span(func_span.clone(), file!(), line!()))?;

            if max_length > ctx.max_string_capacity {
                ctx.max_string_capacity = max_length;
            }

            emitter.emit_str_init(data_offset, max_length);

            let op_type = DEFAULT_OP_TYPE;
            compile_expr(emitter, ctx, arg, op_type)?;
            emitter.emit_str_store_var(data_offset);

            Ok(data_offset)
        }
        ExprKind::Const(ConstantKind::CharacterString(lit)) => {
            // Allocate space in the data region for this string literal.
            let bytes: Vec<u8> = lit.value.iter().map(|&ch| ch as u8).collect();
            let max_length = DEFAULT_STRING_MAX_LENGTH_U16;
            let data_offset = ctx.data_region_offset;
            let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
            ctx.data_region_offset = ctx
                .data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| Diagnostic::todo_with_span(func_span.clone(), file!(), line!()))?;

            if max_length > ctx.max_string_capacity {
                ctx.max_string_capacity = max_length;
            }

            // Emit initialization: header + value.
            emitter.emit_str_init(data_offset, max_length);

            let pool_index = ctx.add_str_constant(bytes);
            ctx.num_temp_bufs += 1;
            emitter.emit_load_const_str(pool_index);
            emitter.emit_str_store_var(data_offset);

            Ok(data_offset)
        }
        _ => {
            // General expression (e.g., nested function call like MID(...)).
            // Compile the expression (pushes buf_idx), then store into a
            // temporary data region slot so the caller gets a data_offset.
            let max_length = DEFAULT_STRING_MAX_LENGTH_U16;
            let data_offset = ctx.data_region_offset;
            let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
            ctx.data_region_offset = ctx
                .data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| Diagnostic::todo_with_span(func_span.clone(), file!(), line!()))?;

            if max_length > ctx.max_string_capacity {
                ctx.max_string_capacity = max_length;
            }

            emitter.emit_str_init(data_offset, max_length);

            let op_type = DEFAULT_OP_TYPE;
            compile_expr(emitter, ctx, arg, op_type)?;
            emitter.emit_str_store_var(data_offset);

            Ok(data_offset)
        }
    }
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

/// Compiles the FIND standard function call.
///
/// FIND(IN1, IN2) returns the 1-based position of the first occurrence
/// of IN2 within IN1, or 0 if not found. Both arguments must be STRING
/// variables.
pub(crate) fn compile_find(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in1_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
    let in2_offset = resolve_string_arg(emitter, ctx, args[1], &func.name.span())?;

    emitter.emit_find_str(in1_offset, in2_offset);
    Ok(())
}

/// Compiles the REPLACE standard function call.
///
/// REPLACE(IN1, IN2, L, P) deletes L characters from IN1 starting at
/// position P (1-based), inserts IN2 in their place, and returns the
/// result as a new string. IN1 and IN2 must be STRING variables; L and P
/// are integer expressions compiled onto the stack.
pub(crate) fn compile_replace(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 4 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in1_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
    let in2_offset = resolve_string_arg(emitter, ctx, args[1], &func.name.span())?;

    // Compile L and P integer expressions onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[2], op_type)?;
    compile_expr(emitter, ctx, args[3], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_replace_str(in1_offset, in2_offset);
    Ok(())
}

/// Compiles the INSERT standard function call.
///
/// INSERT(IN1, IN2, P) inserts string IN2 into IN1 after position P
/// (1-based) and returns the result as a new string. IN1 and IN2 must
/// be STRING variables; P is an integer expression compiled onto the stack.
pub(crate) fn compile_insert(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 3 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in1_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
    let in2_offset = resolve_string_arg(emitter, ctx, args[1], &func.name.span())?;

    // Compile P integer expression onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[2], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_insert_str(in1_offset, in2_offset);
    Ok(())
}

/// Compiles the DELETE standard function call.
///
/// DELETE(IN1, L, P) deletes L characters from IN1 starting at position
/// P (1-based) and returns the result as a new string. IN1 must be a
/// STRING variable; L and P are integer expressions compiled onto the stack.
pub(crate) fn compile_delete(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 3 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in1_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;

    // Compile L and P integer expressions onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[1], op_type)?;
    compile_expr(emitter, ctx, args[2], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_delete_str(in1_offset);
    Ok(())
}

/// Compiles the LEFT standard function call.
///
/// LEFT(IN, L) returns the leftmost L characters of IN. IN must be a
/// STRING variable; L is an integer expression compiled onto the stack.
pub(crate) fn compile_left(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;

    // Compile L integer expression onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[1], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_left_str(in_offset);
    Ok(())
}

/// Compiles the RIGHT standard function call.
///
/// RIGHT(IN, L) returns the rightmost L characters of IN. IN must be a
/// STRING variable; L is an integer expression compiled onto the stack.
pub(crate) fn compile_right(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;

    // Compile L integer expression onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[1], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_right_str(in_offset);
    Ok(())
}

/// Compiles the MID standard function call.
///
/// MID(IN, L, P) returns L characters from IN starting at position P
/// (1-based). IN must be a STRING variable; L and P are integer
/// expressions compiled onto the stack.
pub(crate) fn compile_mid(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 3 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;

    // Compile L and P integer expressions onto the stack.
    let op_type = DEFAULT_OP_TYPE;
    compile_expr(emitter, ctx, args[1], op_type)?;
    compile_expr(emitter, ctx, args[2], op_type)?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_mid_str(in_offset);
    Ok(())
}

/// Compiles the CONCAT standard function call.
///
/// CONCAT(IN1, IN2) concatenates IN1 and IN2. Both arguments must be
/// STRING variables.
pub(crate) fn compile_concat(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Result<(), Diagnostic> {
    let args = collect_positional_args(func);

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let in1_offset = resolve_string_arg(emitter, ctx, args[0], &func.name.span())?;
    let in2_offset = resolve_string_arg(emitter, ctx, args[1], &func.name.span())?;

    // Account for the temp buffer needed for the result.
    ctx.num_temp_bufs += 1;

    emitter.emit_concat_str(in1_offset, in2_offset);
    Ok(())
}
