//! Expression compilation for IEC 61131-3 code generation.
//!
//! Contains expression dispatch, constant compilation, variable reads,
//! and typed opcode emission helpers. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_container::{opcode, VarIndex};
use ironplc_dsl::common::{
    Boolean, ConstantKind, ElementaryTypeName, GenericTypeName, SignedInteger,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    ArrayVariable, BitAccessVariable, CompareOp, Expr, ExprKind, Operator, StructuredVariable,
    SymbolicVariableKind, UnaryOp, Variable,
};
use ironplc_problems::Problem;

use super::compile::{CompileContext, OpType, OpWidth, Signedness, VarTypeInfo, DEFAULT_OP_TYPE};
use super::compile_call::compile_function_call;
use super::compile_setup::resolve_type_name;
use super::compile_string::compile_string_compare;
use crate::emit::Emitter;

/// Returns the operation type from an expression's resolved type annotation.
///
/// The analyzer must have populated `expr.resolved_type`. A missing or
/// unrecognized resolved type is a compiler bug.
pub(crate) fn op_type(expr: &Expr) -> Result<OpType, Diagnostic> {
    let resolved = expr
        .resolved_type
        .as_ref()
        .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    // Enum types resolve to user-defined names (e.g. "COLOR") which
    // resolve_type_name doesn't handle. Fall back to DINT since all
    // enums use W32/Signed at codegen level (REQ-EN-003).
    let info =
        resolve_type_name(&resolved.name).unwrap_or(crate::compile_enum::enum_var_type_info());
    Ok((info.op_width, info.signedness))
}

/// Returns the operation type from an expression's resolved type, if available.
///
/// Unlike [`op_type`] this returns `None` instead of an error when the
/// resolved type is missing or unrecognized, making it safe to use as a
/// best-effort fallback.
pub(crate) fn op_type_from_expr(expr: &Expr) -> Option<OpType> {
    let resolved = expr.resolved_type.as_ref()?;
    let info = resolve_type_name(&resolved.name)?;
    Some((info.op_width, info.signedness))
}

/// Returns the operation type only when the expression has a concrete
/// (non-generic) resolved type.
///
/// Generic types like `ANY_INT` map to a signed default (`DINT`) which is
/// wrong when the other operand is unsigned (e.g. `DWORD`). Returning
/// `None` for generic types lets callers prefer a concrete type from
/// another operand.
pub(crate) fn concrete_op_type_from_expr(expr: &Expr) -> Option<OpType> {
    let resolved = expr.resolved_type.as_ref()?;
    if GenericTypeName::try_from(&resolved.name).is_ok() {
        return None;
    }
    let info = resolve_type_name(&resolved.name)?;
    Some((info.op_width, info.signedness))
}

/// Returns `true` if the expression's resolved type is STRING.
pub(crate) fn expr_is_string(expr: &Expr) -> bool {
    expr.resolved_type
        .as_ref()
        .and_then(|t| ElementaryTypeName::try_from(&t.name).ok())
        .is_some_and(|e| matches!(e, ElementaryTypeName::STRING))
}

/// Returns the storage bit width from an expression's resolved type annotation.
///
/// The analyzer must have populated `expr.resolved_type`. A missing or
/// unrecognized resolved type is a compiler bug.
pub(crate) fn storage_bits(expr: &Expr) -> Result<u8, Diagnostic> {
    let resolved = expr
        .resolved_type
        .as_ref()
        .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    let info =
        resolve_type_name(&resolved.name).ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    Ok(info.storage_bits)
}

/// Returns the operation type for compiling a condition expression.
///
/// For comparison operators (`>`, `<`, `=`, etc.), returns the type of the
/// left operand since the comparison's own resolved type is BOOL but we need
/// the operand type for correct signedness. For boolean combinations (AND,
/// OR, XOR), recurses into the first operand. For other expressions (bare
/// boolean variables, parenthesized expressions), returns the expression's
/// own resolved type.
pub(crate) fn condition_op_type(expr: &Expr) -> Result<OpType, Diagnostic> {
    match &expr.kind {
        ExprKind::Compare(compare) => match compare.op {
            CompareOp::And | CompareOp::Or | CompareOp::Xor => condition_op_type(&compare.left),
            _ => {
                // String comparisons take a dedicated path in compile_expr
                // that emits an i32 boolean; the operand op_type is unused.
                if expr_is_string(&compare.left) {
                    return Ok(DEFAULT_OP_TYPE);
                }
                op_type(&compare.left)
            }
        },
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Not => condition_op_type(&unary.term),
        ExprKind::Expression(inner) => condition_op_type(inner),
        _ => op_type(expr),
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
        ExprKind::Const(constant) => compile_constant(emitter, ctx, constant, op_type),
        ExprKind::Variable(variable) => compile_variable_read(emitter, ctx, variable, op_type),
        ExprKind::BinaryOp(binary) => {
            compile_expr(emitter, ctx, &binary.left, op_type)?;
            compile_expr(emitter, ctx, &binary.right, op_type)?;
            match binary.op {
                Operator::Add => emit_add(emitter, op_type),
                Operator::Sub => emit_sub(emitter, op_type),
                Operator::Mul => emit_mul(emitter, op_type),
                Operator::Div => emit_div(emitter, op_type),
                Operator::Mod => emit_mod(emitter, op_type),
                Operator::Pow => emit_pow(emitter, op_type),
            }
            Ok(())
        }
        ExprKind::UnaryOp(unary) => match unary.op {
            UnaryOp::Neg => {
                compile_expr(emitter, ctx, &unary.term, op_type)?;
                emit_neg(emitter, op_type);
                Ok(())
            }
            UnaryOp::Not => {
                compile_expr(emitter, ctx, &unary.term, op_type)?;
                match op_type {
                    (OpWidth::W32, Signedness::Unsigned) => {
                        emitter.emit_bit_not_32();
                        match storage_bits(&unary.term)? {
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
        },
        ExprKind::LateBound(late_bound) => {
            let var_index = ctx.var_index(&late_bound.value)?;
            emit_load_var(emitter, var_index, op_type);
            Ok(())
        }
        ExprKind::Expression(inner) => compile_expr(emitter, ctx, inner, op_type),
        ExprKind::Compare(compare) => {
            // String comparisons need a completely different code path because
            // strings live in the data region, not on the operand stack.
            if expr_is_string(&compare.left) {
                return compile_string_compare(emitter, ctx, compare);
            }

            // A comparison's result is BOOL, but its operands may be a different
            // type (e.g. REAL for `in < 0.0`). Derive the operand type from a
            // concrete (non-generic) resolved type, preferring the left operand.
            // When one side is a literal (generic type like ANY_INT) and the other
            // is a typed variable (e.g. DWORD), we use the concrete type to ensure
            // correct signedness. This also applies to AND/OR/XOR which can be
            // either boolean (BOOL operands) or bitwise (e.g. DWORD operands).
            let operand_op_type = concrete_op_type_from_expr(&compare.left)
                .or_else(|| concrete_op_type_from_expr(&compare.right))
                .or_else(|| op_type_from_expr(&compare.left))
                .unwrap_or(op_type);
            compile_expr(emitter, ctx, &compare.left, operand_op_type)?;
            compile_expr(emitter, ctx, &compare.right, operand_op_type)?;
            match compare.op {
                CompareOp::Eq => emit_eq(emitter, operand_op_type),
                CompareOp::Ne => emit_ne(emitter, operand_op_type),
                CompareOp::Lt => emit_lt(emitter, operand_op_type),
                CompareOp::Gt => emit_gt(emitter, operand_op_type),
                CompareOp::LtEq => emit_le(emitter, operand_op_type),
                CompareOp::GtEq => emit_ge(emitter, operand_op_type),
                CompareOp::And => emit_and(emitter, operand_op_type),
                CompareOp::Or => emit_or(emitter, operand_op_type),
                CompareOp::Xor => emit_xor(emitter, operand_op_type),
            }
            Ok(())
        }
        ExprKind::EnumeratedValue(enum_val) => {
            // REQ-EN-030: Push the enum value's ordinal as an i32 constant.
            let ordinal = crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, enum_val)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            Ok(())
        }
        ExprKind::Function(func) => compile_function_call(emitter, ctx, func, op_type),
        ExprKind::Ref(variable) => {
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
        ExprKind::Null(_) => {
            // NULL → push null sentinel (u64::MAX) as a u64 constant.
            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
            emitter.emit_load_const_i64(pool_index);
            Ok(())
        }
    }
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
            _ => Err(Diagnostic::todo(file!(), line!())),
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
            let bytes: Vec<u8> = lit.value.iter().map(|&ch| ch as u8).collect();
            let pool_index = ctx.add_str_constant(bytes);
            ctx.num_temp_bufs += 1;
            emitter.emit_load_const_str(pool_index);
            Ok(())
        }
        ConstantKind::Duration(lit) => {
            match op_type.0 {
                OpWidth::W64 => {
                    let milliseconds = lit.interval.whole_milliseconds() as i64;
                    let pool_index = ctx.add_i64_constant(milliseconds);
                    emitter.emit_load_const_i64(pool_index);
                }
                _ => {
                    let milliseconds = lit.interval.whole_milliseconds() as i32;
                    let pool_index = ctx.add_i32_constant(milliseconds);
                    emitter.emit_load_const_i32(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::TimeOfDay(lit) => {
            let ms = lit.whole_milliseconds();
            match op_type.0 {
                OpWidth::W64 => {
                    let pool_index = ctx.add_i64_constant(ms as i64);
                    emitter.emit_load_const_i64(pool_index);
                }
                _ => {
                    let pool_index = ctx.add_i32_constant(ms as i32);
                    emitter.emit_load_const_i32(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::Date(lit) => {
            let secs = lit.seconds_since_epoch();
            match op_type.0 {
                OpWidth::W64 => {
                    let pool_index = ctx.add_i64_constant(secs as i64);
                    emitter.emit_load_const_i64(pool_index);
                }
                _ => {
                    let pool_index = ctx.add_i32_constant(secs as i32);
                    emitter.emit_load_const_i32(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::DateAndTime(lit) => {
            let secs = lit.seconds_since_epoch();
            match op_type.0 {
                OpWidth::W64 => {
                    let pool_index = ctx.add_i64_constant(secs as i64);
                    emitter.emit_load_const_i64(pool_index);
                }
                _ => {
                    let pool_index = ctx.add_i32_constant(secs as i32);
                    emitter.emit_load_const_i32(pool_index);
                }
            }
            Ok(())
        }
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

/// Compiles a variable read expression, handling bit access.
///
/// For simple named variables, loads the variable value onto the stack.
/// For bit access (e.g., `a.0`), loads the base variable, shifts right
/// by the bit index, and masks with 1 to extract the single bit as a
/// BOOL (0 or 1).
pub(crate) fn compile_variable_read(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    variable: &Variable,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::BitAccess(bit_access)) => {
            let bit_index = bit_access.index.value;

            // Determine the op_type of the inner integer value that we are
            // bit-accessing. For a named scalar, look it up in var_types. For
            // an array element, the element type is stored on ArrayVarInfo.
            // For a struct field, walk the struct chain and derive the op
            // type from the leaf field's IntermediateType.
            let base_op_type: OpType = match bit_access.variable.as_ref() {
                SymbolicVariableKind::Named(named) => ctx.var_op_type(&named.name),
                SymbolicVariableKind::Array(array) => {
                    let root_name = resolve_symbolic_variable_name(&array.subscripted_variable)?;
                    match ctx.array_vars.get(root_name) {
                        Some(info) => (
                            info.element_var_type_info.op_width,
                            info.element_var_type_info.signedness,
                        ),
                        None => DEFAULT_OP_TYPE,
                    }
                }
                SymbolicVariableKind::Structured(structured) => {
                    let (_root, _slot, field_type) = crate::compile_struct::walk_struct_chain(
                        ctx,
                        &structured.record,
                        &structured.field,
                        0,
                    )?;
                    crate::compile_struct::resolve_field_op_type(&field_type)
                        .unwrap_or(DEFAULT_OP_TYPE)
                }
                _ => DEFAULT_OP_TYPE,
            };

            // Compile the inner variable read. For a named variable this is
            // emit_load_var; for an array element it is emit_flat_index +
            // emit_load_array; etc. The existing compile_variable_read path
            // already handles each of these.
            let inner_variable: Variable = (*bit_access.variable.clone()).into();
            compile_variable_read(emitter, ctx, &inner_variable, base_op_type)?;

            // Load the bit index and shift right
            match base_op_type.0 {
                OpWidth::W64 => {
                    let pool_index = ctx.add_i64_constant(bit_index as i64);
                    emitter.emit_load_const_i64(pool_index);
                    emitter.emit_builtin(opcode::builtin::SHR_I64);
                    // AND with 1 to isolate the bit
                    let one_index = ctx.add_i64_constant(1);
                    emitter.emit_load_const_i64(one_index);
                    emitter.emit_bit_and_64();
                }
                _ => {
                    let pool_index = ctx.add_i32_constant(bit_index as i32);
                    emitter.emit_load_const_i32(pool_index);
                    emitter.emit_builtin(opcode::builtin::SHR_I32);
                    // AND with 1 to isolate the bit
                    let one_index = ctx.add_i32_constant(1);
                    emitter.emit_load_const_i32(one_index);
                    emitter.emit_bit_and_32();
                }
            }
            Ok(())
        }
        Variable::Symbolic(SymbolicVariableKind::Structured(structured)) => {
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
                ironplc_analyzer::intermediate_type::IntermediateType::String { .. }
            ) {
                let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::NotImplemented,
                        Label::span(
                            structured.span(),
                            format!("Variable '{}' is not a structure", root_name),
                        ),
                    )
                })?;
                let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
                ctx.num_temp_bufs += 1;
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
                    ctx.num_temp_bufs += 1;
                    emitter.emit_str_load_var(data_offset);
                    return Ok(());
                }
            }

            match crate::compile_array::resolve_access(ctx, variable)? {
                crate::compile_array::ResolvedAccess::Scalar { var_index } => {
                    emit_load_var(emitter, var_index, op_type);
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
                        ctx.num_temp_bufs += 1;
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
                crate::compile_array::ResolvedAccess::StructFieldStringArrayElement {
                    var_index,
                    scratch_var_index,
                    string_desc_index,
                    field_byte_offset,
                    ref dimensions,
                    subscripts,
                } => {
                    let span = variable_span(variable);
                    // 1. Compute base: struct_data_offset + field_byte_offset → scratch.
                    emitter.emit_load_var_i32(var_index);
                    let offset_const = ctx.add_i32_constant(field_byte_offset as i32);
                    emitter.emit_load_const_i32(offset_const);
                    emitter.emit_add_i32();
                    emitter.emit_store_var_i32(scratch_var_index);
                    // 2. Compute flat index.
                    crate::compile_array::emit_flat_index(
                        emitter,
                        ctx,
                        &subscripts,
                        dimensions,
                        &span,
                    )?;
                    // 3. Load string element.
                    ctx.num_temp_bufs += 1;
                    emitter.emit_str_load_array_elem(scratch_var_index, string_desc_index);
                }
            }
            Ok(())
        }
    }
}

/// Resolves the name of the innermost named variable from a symbolic variable kind.
pub(crate) fn resolve_symbolic_variable_name(
    kind: &SymbolicVariableKind,
) -> Result<&Id, Diagnostic> {
    match kind {
        SymbolicVariableKind::Named(named) => Ok(&named.name),
        SymbolicVariableKind::BitAccess(bit_access) => {
            resolve_symbolic_variable_name(&bit_access.variable)
        }
        SymbolicVariableKind::Array(array) => {
            resolve_symbolic_variable_name(&array.subscripted_variable)
        }
        SymbolicVariableKind::Structured(structured) => {
            resolve_symbolic_variable_name(&structured.record)
        }
        SymbolicVariableKind::Deref(deref) => resolve_symbolic_variable_name(&deref.variable),
    }
}

/// Resolves a variable reference to its variable table index.
pub(crate) fn resolve_variable(
    ctx: &CompileContext,
    variable: &Variable,
) -> Result<VarIndex, Diagnostic> {
    match variable {
        Variable::Symbolic(symbolic) => match symbolic {
            SymbolicVariableKind::Named(named) => ctx.var_index(&named.name),
            SymbolicVariableKind::Array(array) => {
                Err(Diagnostic::todo_with_span(array.span(), file!(), line!()))
            }
            SymbolicVariableKind::Structured(structured) => Err(Diagnostic::todo_with_span(
                structured.span(),
                file!(),
                line!(),
            )),
            SymbolicVariableKind::BitAccess(bit_access) => Err(Diagnostic::todo_with_span(
                bit_access.span(),
                file!(),
                line!(),
            )),
            SymbolicVariableKind::Deref(deref) => {
                Err(Diagnostic::todo_with_span(deref.span(), file!(), line!()))
            }
        },
        Variable::Direct(direct) => Err(Diagnostic::todo_with_span(
            direct.position.clone(),
            file!(),
            line!(),
        )),
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

/// Extracts a `BitAccessVariable` from an assignment target, if it is a bit access.
pub(crate) fn extract_bit_access_target(variable: &Variable) -> Option<&BitAccessVariable> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::BitAccess(bit_access)) => Some(bit_access),
        _ => None,
    }
}

/// Compiles a bit access assignment using read-modify-write.
///
/// `target.N := value` is compiled as:
///   1. Load the base variable
///   2. AND with clear mask (~(1 << N)) to clear the target bit
///   3. Compile the RHS value
///   4. AND with 1 to ensure it's 0 or 1
///   5. Left-shift by N
///   6. OR with the cleared variable to set the bit
///   7. Store back to the base variable
pub(crate) fn compile_bit_access_assignment(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    bit_access: &BitAccessVariable,
    value: &Expr,
) -> Result<(), Diagnostic> {
    // Array-element base: use the array read/write opcodes which take the
    // flat index on the stack. The index is emitted twice (once for the
    // load, once for the store) because no DUP opcode is available.
    if let SymbolicVariableKind::Array(array) = bit_access.variable.as_ref() {
        return compile_bit_access_assignment_on_array(emitter, ctx, array, bit_access, value);
    }

    // Struct-field base: `s.field.n := rhs;`. Uses LOAD_ARRAY/STORE_ARRAY on
    // the underlying struct-as-flat-slot-array with a compile-time slot index.
    if let SymbolicVariableKind::Structured(structured) = bit_access.variable.as_ref() {
        return compile_bit_access_assignment_on_struct_field(
            emitter, ctx, structured, bit_access, value,
        );
    }

    let base_name = resolve_symbolic_variable_name(&bit_access.variable)?;
    let var_index = ctx.var_index(base_name)?;
    let base_op_type = ctx.var_op_type(base_name);
    let bit_index = bit_access.index.value as u32;

    match base_op_type.0 {
        OpWidth::W64 => {
            let clear_mask = !(1i64 << bit_index);
            let clear_pool = ctx.add_i64_constant(clear_mask);

            // Load base var and clear the target bit.
            emit_load_var(emitter, var_index, base_op_type);
            emitter.emit_load_const_i64(clear_pool);
            emitter.emit_bit_and_64();

            // Compile the RHS, mask to 1 bit, shift into position.
            compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
            let one_pool = ctx.add_i32_constant(1);
            emitter.emit_load_const_i32(one_pool);
            emitter.emit_bit_and_32();
            // Widen to 64-bit before shifting.
            emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
            let shift_pool = ctx.add_i32_constant(bit_index as i32);
            emitter.emit_load_const_i32(shift_pool);
            emitter.emit_builtin(opcode::builtin::SHL_I64);

            // OR the shifted bit into the cleared variable.
            emitter.emit_bit_or_64();
        }
        _ => {
            let clear_mask = !(1i32 << bit_index);
            let clear_pool = ctx.add_i32_constant(clear_mask);

            // Load base var and clear the target bit.
            emit_load_var(emitter, var_index, base_op_type);
            emitter.emit_load_const_i32(clear_pool);
            emitter.emit_bit_and_32();

            // Compile the RHS, mask to 1 bit, shift into position.
            compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
            let one_pool = ctx.add_i32_constant(1);
            emitter.emit_load_const_i32(one_pool);
            emitter.emit_bit_and_32();
            let shift_pool = ctx.add_i32_constant(bit_index as i32);
            emitter.emit_load_const_i32(shift_pool);
            emitter.emit_builtin(opcode::builtin::SHL_I32);

            // OR the shifted bit into the cleared variable.
            emitter.emit_bit_or_32();
        }
    }

    // Truncate if needed and store back.
    if let Some(ti) = ctx.var_type_info(base_name) {
        emit_truncation(emitter, ti);
    }
    emit_store_var(emitter, var_index, base_op_type);
    Ok(())
}

/// Compiles a bit-access assignment where the base is an array element:
/// `arr[i].n := rhs;`. Uses LOAD_ARRAY/STORE_ARRAY; the flat index is emitted
/// twice (once for the read, once for the write) because no DUP opcode is
/// available. Supports both W32 element widths (BYTE/WORD/DWORD, INT/DINT)
/// and W64 element widths (LWORD/LINT/ULINT).
fn compile_bit_access_assignment_on_array(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    array: &ArrayVariable,
    bit_access: &BitAccessVariable,
    value: &Expr,
) -> Result<(), Diagnostic> {
    let bit_index = bit_access.index.value as u32;

    let root_name = resolve_symbolic_variable_name(&array.subscripted_variable)?;
    let info = ctx.array_vars.get(root_name).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                bit_access.span(),
                "Bit access on non-trivial array base is not yet supported",
            ),
        )
    })?;
    // Copy scalar fields out of the borrow so ctx can be used mutably.
    let arr_var_index = info.var_index;
    let arr_desc_index = info.desc_index;
    let element_vti = info.element_var_type_info;
    let dim_info: Vec<crate::compile_array::DimensionInfo> = info
        .dimensions
        .iter()
        .map(|d| crate::compile_array::DimensionInfo {
            lower_bound: d.lower_bound,
            size: d.size,
            stride: d.stride,
        })
        .collect();
    let subscripts: Vec<&Expr> = array.subscripts.iter().collect();

    let span = bit_access.span();

    // 1. Compute flat index and load the element.
    crate::compile_array::emit_flat_index(emitter, ctx, &subscripts, &dim_info, &span)?;
    emitter.emit_load_array(arr_var_index, arr_desc_index);

    // 2/3. Clear the target bit, then OR in the shifted RHS bit. Width-
    //      dependent: LWORD/LINT elements use 64-bit ops; everything else
    //      (BYTE/WORD/DWORD/INT/DINT) uses 32-bit ops.
    if element_vti.op_width == OpWidth::W64 {
        let clear_mask = !(1i64 << bit_index);
        let clear_pool = ctx.add_i64_constant(clear_mask);
        emitter.emit_load_const_i64(clear_pool);
        emitter.emit_bit_and_64();

        compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
        let one_pool = ctx.add_i32_constant(1);
        emitter.emit_load_const_i32(one_pool);
        emitter.emit_bit_and_32();
        // Widen to 64-bit before shifting into a high bit position.
        emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        let shift_pool = ctx.add_i32_constant(bit_index as i32);
        emitter.emit_load_const_i32(shift_pool);
        emitter.emit_builtin(opcode::builtin::SHL_I64);
        emitter.emit_bit_or_64();
    } else {
        let clear_mask = !(1i32 << bit_index);
        let clear_pool = ctx.add_i32_constant(clear_mask);
        emitter.emit_load_const_i32(clear_pool);
        emitter.emit_bit_and_32();

        compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
        let one_pool = ctx.add_i32_constant(1);
        emitter.emit_load_const_i32(one_pool);
        emitter.emit_bit_and_32();
        let shift_pool = ctx.add_i32_constant(bit_index as i32);
        emitter.emit_load_const_i32(shift_pool);
        emitter.emit_builtin(opcode::builtin::SHL_I32);
        emitter.emit_bit_or_32();
    }

    // 4. Truncate the new element value to fit its storage width.
    emit_truncation(emitter, element_vti);

    // 5. Recompute the flat index and STORE back.
    crate::compile_array::emit_flat_index(emitter, ctx, &subscripts, &dim_info, &span)?;
    emitter.emit_store_array(arr_var_index, arr_desc_index);

    Ok(())
}

/// Compiles a bit-access assignment where the base is a struct field:
/// `s.field.n := rhs;`. Uses LOAD_ARRAY/STORE_ARRAY on the struct's flat
/// slot-array with the field's compile-time slot offset as the index.
fn compile_bit_access_assignment_on_struct_field(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    structured: &StructuredVariable,
    bit_access: &BitAccessVariable,
    value: &Expr,
) -> Result<(), Diagnostic> {
    let bit_index = bit_access.index.value as u32;

    let (var_index, desc_index, slot_offset, _op_type, field_type) =
        crate::compile_struct::resolve_struct_field_access(ctx, structured)?;
    let field_vti =
        crate::compile_struct::var_type_info_for_field(&field_type).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(
                    structured.field.span(),
                    "Bit access on non-integer struct field is not supported",
                ),
            )
        })?;

    // Index constant is the same for load and store; add once and reuse
    // across both emitter calls.
    let idx_const = ctx.add_i32_constant(slot_offset.raw() as i32);

    // 1. Load the current field value.
    emitter.emit_load_const_i32(idx_const);
    emitter.emit_load_array(var_index, desc_index);

    // 2/3. Clear target bit and OR in the shifted RHS bit.
    if field_vti.op_width == OpWidth::W64 {
        let clear_mask = !(1i64 << bit_index);
        let clear_pool = ctx.add_i64_constant(clear_mask);
        emitter.emit_load_const_i64(clear_pool);
        emitter.emit_bit_and_64();

        compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
        let one_pool = ctx.add_i32_constant(1);
        emitter.emit_load_const_i32(one_pool);
        emitter.emit_bit_and_32();
        emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        let shift_pool = ctx.add_i32_constant(bit_index as i32);
        emitter.emit_load_const_i32(shift_pool);
        emitter.emit_builtin(opcode::builtin::SHL_I64);
        emitter.emit_bit_or_64();
    } else {
        let clear_mask = !(1i32 << bit_index);
        let clear_pool = ctx.add_i32_constant(clear_mask);
        emitter.emit_load_const_i32(clear_pool);
        emitter.emit_bit_and_32();

        compile_expr(emitter, ctx, value, DEFAULT_OP_TYPE)?;
        let one_pool = ctx.add_i32_constant(1);
        emitter.emit_load_const_i32(one_pool);
        emitter.emit_bit_and_32();
        let shift_pool = ctx.add_i32_constant(bit_index as i32);
        emitter.emit_load_const_i32(shift_pool);
        emitter.emit_builtin(opcode::builtin::SHL_I32);
        emitter.emit_bit_or_32();
    }

    // 4. Truncate the new field value to fit its storage width (e.g., SINT
    //    stored in a W32 slot needs sign-extension/truncation).
    emit_truncation(emitter, field_vti);

    // 5. Re-emit the slot index and STORE back.
    emitter.emit_load_const_i32(idx_const);
    emitter.emit_store_array(var_index, desc_index);

    Ok(())
}

/// Converts a `SignedInteger` AST node to an `i64` value.
pub(crate) fn signed_integer_to_i64(si: &SignedInteger) -> Result<i64, Diagnostic> {
    if si.is_neg {
        let unsigned = si.value.value as i128;
        let signed = -unsigned;
        i64::try_from(signed).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &signed.to_string())
        })
    } else {
        i64::try_from(si.value.value).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &si.value.value.to_string())
        })
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

pub(crate) fn emit_add(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_add_i32(),
        OpWidth::W64 => emitter.emit_add_i64(),
        OpWidth::F32 => emitter.emit_add_f32(),
        OpWidth::F64 => emitter.emit_add_f64(),
    }
}

pub(crate) fn emit_sub(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_sub_i32(),
        OpWidth::W64 => emitter.emit_sub_i64(),
        OpWidth::F32 => emitter.emit_sub_f32(),
        OpWidth::F64 => emitter.emit_sub_f64(),
    }
}

pub(crate) fn emit_mul(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_mul_i32(),
        OpWidth::W64 => emitter.emit_mul_i64(),
        OpWidth::F32 => emitter.emit_mul_f32(),
        OpWidth::F64 => emitter.emit_mul_f64(),
    }
}

pub(crate) fn emit_div(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_div_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_div_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_div_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_div_u64(),
        (OpWidth::F32, _) => emitter.emit_div_f32(),
        (OpWidth::F64, _) => emitter.emit_div_f64(),
    }
}

pub(crate) fn emit_mod(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_mod_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_mod_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_mod_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_mod_u64(),
        // Float MOD is not supported (IEC 61131-3 MOD is integer-only).
        // The analyzer should catch this before codegen.
        (OpWidth::F32, _) | (OpWidth::F64, _) => {}
    }
}

pub(crate) fn emit_neg(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_neg_i32(),
        OpWidth::W64 => emitter.emit_neg_i64(),
        OpWidth::F32 => emitter.emit_neg_f32(),
        OpWidth::F64 => emitter.emit_neg_f64(),
    }
}

pub(crate) fn emit_pow(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_builtin(opcode::builtin::EXPT_I32),
        OpWidth::W64 => emitter.emit_builtin(opcode::builtin::EXPT_I64),
        OpWidth::F32 => emitter.emit_builtin(opcode::builtin::EXPT_F32),
        OpWidth::F64 => emitter.emit_builtin(opcode::builtin::EXPT_F64),
    }
}

pub(crate) fn emit_and(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_and_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_and_64(),
        _ => emitter.emit_bool_and(),
    }
}

pub(crate) fn emit_or(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_or_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_or_64(),
        _ => emitter.emit_bool_or(),
    }
}

pub(crate) fn emit_xor(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_xor_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_xor_64(),
        _ => emitter.emit_bool_xor(),
    }
}

pub(crate) fn emit_eq(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_eq_i32(),
        OpWidth::W64 => emitter.emit_eq_i64(),
        OpWidth::F32 => emitter.emit_eq_f32(),
        OpWidth::F64 => emitter.emit_eq_f64(),
    }
}

pub(crate) fn emit_ne(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_ne_i32(),
        OpWidth::W64 => emitter.emit_ne_i64(),
        OpWidth::F32 => emitter.emit_ne_f32(),
        OpWidth::F64 => emitter.emit_ne_f64(),
    }
}

pub(crate) fn emit_lt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_lt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_lt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_lt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_lt_u64(),
        (OpWidth::F32, _) => emitter.emit_lt_f32(),
        (OpWidth::F64, _) => emitter.emit_lt_f64(),
    }
}

pub(crate) fn emit_le(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_le_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_le_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_le_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_le_u64(),
        (OpWidth::F32, _) => emitter.emit_le_f32(),
        (OpWidth::F64, _) => emitter.emit_le_f64(),
    }
}

pub(crate) fn emit_gt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_gt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_gt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_gt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_gt_u64(),
        (OpWidth::F32, _) => emitter.emit_gt_f32(),
        (OpWidth::F64, _) => emitter.emit_gt_f64(),
    }
}

pub(crate) fn emit_ge(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_ge_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_ge_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_ge_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_ge_u64(),
        (OpWidth::F32, _) => emitter.emit_ge_f32(),
        (OpWidth::F64, _) => emitter.emit_ge_f64(),
    }
}
