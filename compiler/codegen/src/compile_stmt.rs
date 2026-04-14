//! Statement and control flow compilation for IEC 61131-3 code generation.
//!
//! Contains statement dispatch, control flow (IF, CASE, FOR, WHILE, REPEAT),
//! and function block call compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_dsl::common::{
    ConstantKind, FunctionBlockBodyKind, IntegerRef, SignedInteger, SignedIntegerRef,
    StringInitializer, StringSpecification,
};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CaseSelectionKind, Expr, ExprKind, FbCall, ParamAssignmentKind, Statements, StmtKind,
    SymbolicVariableKind, UnaryOp, Variable,
};
use ironplc_problems::Problem;

use super::compile::{
    CompileContext, OpType, OpWidth, Signedness, DEFAULT_OP_TYPE, DEFAULT_STRING_MAX_LENGTH_U16,
};
use super::compile_expr::{
    compile_bit_access_assignment, compile_expr, condition_op_type, emit_add, emit_ge, emit_gt,
    emit_le, emit_load_var, emit_lt, emit_store_var, emit_truncation, extract_bit_access_target,
    op_type, resolve_variable, resolve_variable_name, signed_integer_to_i64, variable_span,
};
use crate::emit::Emitter;

/// Compiles a function block body.
pub(crate) fn compile_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &FunctionBlockBodyKind,
) -> Result<(), Diagnostic> {
    match body {
        FunctionBlockBodyKind::Statements(statements) => {
            compile_statements(emitter, ctx, statements)
        }
        FunctionBlockBodyKind::Empty => Ok(()),
        FunctionBlockBodyKind::Sfc(_) => Err(Diagnostic::todo(file!(), line!())),
    }
}

/// Compiles a sequence of statements.
pub(crate) fn compile_statements(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    statements: &Statements,
) -> Result<(), Diagnostic> {
    for stmt in &statements.body {
        compile_statement(emitter, ctx, stmt)?;
    }
    Ok(())
}

/// Compiles a single statement.
fn compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &StmtKind,
) -> Result<(), Diagnostic> {
    match stmt {
        StmtKind::Assignment(assignment) => {
            // Dereference assignment: myRef^ := expr
            // Compile the RHS, load the reference variable, emit STORE_INDIRECT.
            if assignment.deref {
                let target_name = resolve_variable_name(&assignment.target);
                let target_index = target_name
                    .and_then(|name| ctx.variables.get(name).copied())
                    .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;

                // Compile the value expression (use DEFAULT_OP_TYPE; the referenced
                // type determines the actual width at runtime).
                compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;

                // Load the reference (variable index stored in the ref variable).
                emitter.emit_load_var_i64(target_index);

                // STORE_INDIRECT pops both value and ref.
                emitter.emit_store_indirect();
                return Ok(());
            }

            // Check if the target is a bit access variable (read-modify-write).
            if let Some(bit_access) = extract_bit_access_target(&assignment.target) {
                return compile_bit_access_assignment(emitter, ctx, bit_access, &assignment.value);
            }

            // Check if the target is a structured variable (struct field write).
            if let Variable::Symbolic(SymbolicVariableKind::Structured(structured)) =
                &assignment.target
            {
                // STRING fields are composite (multi-slot) and handled via the
                // data region, so we intercept before resolve_struct_field_access
                // which only supports single-slot (primitive/enum) fields.
                let (root_name, slot_offset, field_type) =
                    crate::compile_struct::walk_struct_chain(
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
                    compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;
                    emitter.emit_str_store_var(byte_offset);
                    return Ok(());
                }

                let (var_index, desc_index, slot_offset, op_type, field_type) =
                    crate::compile_struct::resolve_struct_field_access(ctx, structured)?;
                compile_expr(emitter, ctx, &assignment.value, op_type)?;
                crate::compile_struct::emit_truncation_for_field(emitter, &field_type);
                let idx_const = ctx.add_i32_constant(slot_offset.raw() as i32);
                emitter.emit_load_const_i32(idx_const);
                emitter.emit_store_array(var_index, desc_index);
                return Ok(());
            }

            // Check if the target is a struct variable (whole-struct assignment).
            if let Some(target_name) = resolve_variable_name(&assignment.target) {
                if let Some(dst_info) = ctx.struct_vars.get(target_name).cloned() {
                    let dst_var = ctx.var_index(target_name)?;
                    // Compile RHS (for struct-returning functions, leaves
                    // the source struct's data_offset on the stack).
                    compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;

                    // Copy protocol: temporarily point dst_var to source data,
                    // load all fields, restore dst_var, store all fields.
                    emit_store_var(emitter, dst_var, DEFAULT_OP_TYPE);

                    let total = dst_info.total_slots.raw();
                    for i in 0..total {
                        let idx = ctx.add_i32_constant(i as i32);
                        emitter.emit_load_const_i32(idx);
                        emitter.emit_load_array(dst_var, dst_info.desc_index);
                    }

                    // Restore dst_var's own data_offset.
                    let dst_offset = ctx.add_i32_constant(dst_info.data_offset as i32);
                    emitter.emit_load_const_i32(dst_offset);
                    emit_store_var(emitter, dst_var, DEFAULT_OP_TYPE);

                    // Store fields in reverse order (LIFO stack consumption).
                    for i in (0..total).rev() {
                        let idx = ctx.add_i32_constant(i as i32);
                        emitter.emit_load_const_i32(idx);
                        emitter.emit_store_array(dst_var, dst_info.desc_index);
                    }
                    return Ok(());
                }
            }

            // Look up the target variable's type info.
            let target_name = resolve_variable_name(&assignment.target);

            // Check if the target is a STRING variable (stored in data region).
            let string_info =
                target_name.and_then(|name| ctx.string_vars.get(name).map(|info| info.data_offset));

            if let Some(data_offset) = string_info {
                // String target: compile RHS (produces buf_idx), then STR_STORE_VAR.
                let op_type = DEFAULT_OP_TYPE;
                compile_expr(emitter, ctx, &assignment.value, op_type)?;
                emitter.emit_str_store_var(data_offset);
            } else {
                match crate::compile_array::resolve_access(ctx, &assignment.target)? {
                    crate::compile_array::ResolvedAccess::Scalar { var_index } => {
                        let type_info = target_name.and_then(|name| ctx.var_type_info(name));
                        let op_type = type_info
                            .map(|ti| (ti.op_width, ti.signedness))
                            .unwrap_or(DEFAULT_OP_TYPE);
                        compile_expr(emitter, ctx, &assignment.value, op_type)?;
                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }
                        emit_store_var(emitter, var_index, op_type);
                    }
                    crate::compile_array::ResolvedAccess::ArrayElement { info, subscripts } => {
                        // Copy scalar fields from info (borrows ctx) before using ctx mutably.
                        let element_vti = info.element_var_type_info;
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
                        // info is no longer used; subscripts borrows from AST, not ctx.
                        let target_span = variable_span(&assignment.target);

                        if is_string_elem {
                            // String array: compile RHS (produces buf_idx), then flat index,
                            // then STR_STORE_ARRAY_ELEM.
                            compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;
                            crate::compile_array::emit_flat_index(
                                emitter,
                                ctx,
                                &subscripts,
                                &dim_info,
                                &target_span,
                            )?;
                            emitter.emit_str_store_array_elem(arr_var_index, arr_desc_index);
                        } else {
                            let element_op_type = (element_vti.op_width, element_vti.signedness);
                            // 1. Compile the RHS value.
                            compile_expr(emitter, ctx, &assignment.value, element_op_type)?;
                            // 2. Truncate for sub-32-bit types.
                            emit_truncation(emitter, element_vti);
                            // 3. Compute the flat index.
                            crate::compile_array::emit_flat_index(
                                emitter,
                                ctx,
                                &subscripts,
                                &dim_info,
                                &target_span,
                            )?;
                            // Stack: [..., value, index]. STORE_ARRAY pops both.
                            emitter.emit_store_array(arr_var_index, arr_desc_index);
                        }
                    }
                    crate::compile_array::ResolvedAccess::DerefArrayElement {
                        info,
                        subscripts,
                    } => {
                        let element_vti = info.element_var_type_info;
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
                        let element_op_type = (element_vti.op_width, element_vti.signedness);
                        let target_span = variable_span(&assignment.target);

                        compile_expr(emitter, ctx, &assignment.value, element_op_type)?;
                        emit_truncation(emitter, element_vti);
                        crate::compile_array::emit_flat_index(
                            emitter,
                            ctx,
                            &subscripts,
                            &dim_info,
                            &target_span,
                        )?;
                        emitter.emit_store_array_deref(ref_var_index, arr_desc_index);
                    }
                    crate::compile_array::ResolvedAccess::StructFieldArrayElement {
                        var_index,
                        desc_index,
                        field_slot_offset,
                        ref dimensions,
                        subscripts,
                        element_op_type,
                        ref element_type,
                    } => {
                        let target_span = variable_span(&assignment.target);
                        compile_expr(emitter, ctx, &assignment.value, element_op_type)?;
                        crate::compile_struct::emit_truncation_for_field(emitter, element_type);
                        crate::compile_array::emit_flat_index(
                            emitter,
                            ctx,
                            &subscripts,
                            dimensions,
                            &target_span,
                        )?;
                        let offset_const = ctx.add_i64_constant(field_slot_offset.raw() as i64);
                        emitter.emit_load_const_i64(offset_const);
                        emitter.emit_add_i64();
                        emitter.emit_store_array(var_index, desc_index);
                    }
                    crate::compile_array::ResolvedAccess::StructFieldStringArrayElement {
                        var_index,
                        scratch_var_index,
                        string_desc_index,
                        field_byte_offset,
                        ref dimensions,
                        subscripts,
                    } => {
                        let target_span = variable_span(&assignment.target);
                        // 1. Compile RHS (produces buf_idx on stack).
                        compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;
                        // 2. Compute base: struct_data_offset + field_byte_offset → scratch.
                        emitter.emit_load_var_i32(var_index);
                        let offset_const = ctx.add_i32_constant(field_byte_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_add_i32();
                        emitter.emit_store_var_i32(scratch_var_index);
                        // 3. Compute flat index.
                        crate::compile_array::emit_flat_index(
                            emitter,
                            ctx,
                            &subscripts,
                            dimensions,
                            &target_span,
                        )?;
                        // 4. Store string element.
                        emitter.emit_str_store_array_elem(scratch_var_index, string_desc_index);
                    }
                }
            }
            Ok(())
        }
        StmtKind::FbCall(fb_call) => compile_fb_call(emitter, ctx, fb_call),
        StmtKind::If(if_stmt) => compile_if(emitter, ctx, if_stmt),
        StmtKind::Case(case_stmt) => compile_case(emitter, ctx, case_stmt),
        StmtKind::For(for_stmt) => compile_for(emitter, ctx, for_stmt),
        StmtKind::While(while_stmt) => compile_while(emitter, ctx, while_stmt),
        StmtKind::Repeat(repeat_stmt) => compile_repeat(emitter, ctx, repeat_stmt),
        StmtKind::Return => {
            emitter.emit_ret_void();
            Ok(())
        }
        StmtKind::Exit(span) => {
            let label = ctx.current_loop_exit().ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ExitOutsideLoop,
                    Label::span(
                        span.clone(),
                        "EXIT must be inside a FOR, WHILE, or REPEAT loop",
                    ),
                )
            })?;
            emitter.emit_jmp(label);
            Ok(())
        }
    }
}

/// Returns the op_type for an FB field, checking user-defined FBs first,
/// then falling back to the stdlib hardcoded mapping.
fn resolve_fb_field_op_type(ctx: &CompileContext, type_id: u16, field_name: &str) -> OpType {
    // Check user-defined FBs by type_id.
    for user_fb in ctx.user_fb_types.values() {
        if user_fb.type_id == type_id {
            if let Some(op_type) = user_fb.field_op_types.get(field_name) {
                return *op_type;
            }
        }
    }
    // Fall back to stdlib field names.
    fb_field_op_type(field_name)
}

/// Compiles a function block invocation: stores inputs, calls FB, reads outputs.
fn compile_fb_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    fb_call: &FbCall,
) -> Result<(), Diagnostic> {
    let fb_info = ctx
        .fb_instances
        .get(&fb_call.var_name)
        .ok_or_else(|| Diagnostic::todo_with_span(fb_call.span(), file!(), line!()))?;
    let type_id = fb_info.type_id;
    let field_indices = fb_info.field_indices.clone();
    let var_index = fb_info.var_index;

    // Push FB instance reference.
    emitter.emit_fb_load_instance(var_index);

    // Store input parameters.
    for param in &fb_call.params {
        if let ParamAssignmentKind::NamedInput(input) = param {
            let field_name = input.name.to_string().to_lowercase();
            let field_idx = field_indices
                .get(&field_name)
                .ok_or_else(|| Diagnostic::todo_with_span(input.name.span(), file!(), line!()))?;
            let op_type = resolve_fb_field_op_type(ctx, type_id, &field_name);
            compile_expr(emitter, ctx, &input.expr, op_type)?;
            emitter.emit_fb_store_param(*field_idx);
        }
    }

    // Call the function block.
    emitter.emit_fb_call(type_id);

    // Read output parameters.
    for param in &fb_call.params {
        if let ParamAssignmentKind::Output(output) = param {
            let field_name = output.src.to_string().to_lowercase();
            let field_idx = field_indices
                .get(&field_name)
                .ok_or_else(|| Diagnostic::todo_with_span(output.src.span(), file!(), line!()))?;
            emitter.emit_fb_load_param(*field_idx);
            let target_index = resolve_variable(ctx, &output.tgt)?;
            let op_type = resolve_fb_field_op_type(ctx, type_id, &field_name);
            emit_store_var(emitter, target_index, op_type);
        }
    }

    // Discard fb_ref.
    emitter.emit_pop();
    Ok(())
}

/// Returns the op_type for a standard FB field by name.
fn fb_field_op_type(field_name: &str) -> OpType {
    match field_name {
        "in" | "q" => (OpWidth::W32, Signedness::Signed),
        "pt" | "et" => (OpWidth::W32, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    }
}

/// Compiles a slice of statements.
fn compile_stmts(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmts: &[StmtKind],
) -> Result<(), Diagnostic> {
    for stmt in stmts {
        compile_statement(emitter, ctx, stmt)?;
    }
    Ok(())
}

/// Compiles an IF/ELSIF/ELSE statement.
fn compile_if(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    if_stmt: &ironplc_dsl::textual::If,
) -> Result<(), Diagnostic> {
    let has_else_ifs = !if_stmt.else_ifs.is_empty();
    let has_else = !if_stmt.else_body.is_empty();
    let needs_end_label = has_else_ifs || has_else;

    let end_label = if needs_end_label {
        Some(emitter.create_label())
    } else {
        None
    };

    // Compile the IF condition at its inferred operation type.
    let cond_type = condition_op_type(&if_stmt.expr)?;
    compile_expr(emitter, ctx, &if_stmt.expr, cond_type)?;

    // Jump past the then-body if condition is false.
    let next_label = emitter.create_label();
    emitter.emit_jmp_if_not(next_label);

    // Compile the then-body.
    compile_stmts(emitter, ctx, &if_stmt.body)?;

    // If there are more branches, jump to end.
    if needs_end_label {
        emitter.emit_jmp(end_label.unwrap());
    }

    emitter.bind_label(next_label);

    // Compile ELSIF clauses.
    for elsif in &if_stmt.else_ifs {
        let elsif_op_type = condition_op_type(&elsif.expr)?;
        compile_expr(emitter, ctx, &elsif.expr, elsif_op_type)?;
        let elsif_next = emitter.create_label();
        emitter.emit_jmp_if_not(elsif_next);

        compile_stmts(emitter, ctx, &elsif.body)?;

        emitter.emit_jmp(end_label.unwrap());

        emitter.bind_label(elsif_next);
    }

    // Compile ELSE body (if present).
    if has_else {
        compile_stmts(emitter, ctx, &if_stmt.else_body)?;
    }

    // Bind the end label.
    if let Some(end) = end_label {
        emitter.bind_label(end);
    }

    Ok(())
}

/// Compiles a CASE statement.
///
/// Each `CaseStatementGroup` is compiled as a chain of comparisons (like
/// IF/ELSIF/ELSE). Multi-value selectors are OR'd together.
///
/// ```text
///   // For each arm:
///   compile(selector)
///   LOAD_CONST case_value
///   EQ_I32
///   JMP_IF_NOT → next_arm
///   compile(body)
///   JMP → END
/// next_arm:
///   // ... next arm / ELSE body ...
/// END:
/// ```
fn compile_case(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    case_stmt: &ironplc_dsl::textual::Case,
) -> Result<(), Diagnostic> {
    let end_label = emitter.create_label();
    // Enum selectors have a resolved type that is the enum name (e.g. "COLOR"),
    // which resolve_type_name doesn't handle. Fall back to W32/Signed (DINT)
    // since all enums use DINT at codegen level (REQ-EN-003).
    let op_type = op_type(&case_stmt.selector).unwrap_or(crate::compile::DEFAULT_OP_TYPE);

    for group in &case_stmt.statement_groups {
        let next_label = emitter.create_label();

        // Compile selector comparisons with OR logic.
        for (i, selection) in group.selectors.iter().enumerate() {
            compile_case_selector(emitter, ctx, &case_stmt.selector, selection, op_type)?;
            if i > 0 {
                emitter.emit_bool_or();
            }
        }

        emitter.emit_jmp_if_not(next_label);

        // Compile body.
        compile_stmts(emitter, ctx, &group.statements)?;

        emitter.emit_jmp(end_label);

        emitter.bind_label(next_label);
    }

    // Compile ELSE body if present.
    compile_stmts(emitter, ctx, &case_stmt.else_body)?;

    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles a single case selector, leaving a boolean result on the stack.
///
/// - `SignedInteger`: `selector == value`
/// - `Subrange`: `(selector >= start) AND (selector <= end)`
/// - `EnumeratedValue`: not yet supported (returns todo diagnostic)
fn compile_case_selector(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    selector_expr: &Expr,
    selection: &CaseSelectionKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match selection {
        CaseSelectionKind::SignedInteger(si) => {
            compile_expr(emitter, ctx, selector_expr, op_type)?;
            match op_type.0 {
                OpWidth::W32 => {
                    let value = signed_integer_to_i32(si)?;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                    emitter.emit_eq_i32();
                }
                OpWidth::W64 => {
                    let value = signed_integer_to_i64(si)?;
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                    emitter.emit_eq_i64();
                }
                // CASE with float types is not meaningful in IEC 61131-3.
                _ => return Err(Diagnostic::todo(file!(), line!())),
            }
            Ok(())
        }
        CaseSelectionKind::Subrange(sr) => {
            // (selector >= start) AND (selector <= end)
            compile_expr(emitter, ctx, selector_expr, op_type)?;
            match op_type.0 {
                OpWidth::W32 => {
                    let start_si = resolve_signed_integer_ref(&sr.start)?;
                    let start = signed_integer_to_i32(start_si)?;
                    let start_index = ctx.add_i32_constant(start);
                    emitter.emit_load_const_i32(start_index);
                    emit_ge(emitter, op_type);

                    compile_expr(emitter, ctx, selector_expr, op_type)?;
                    let end_si = resolve_signed_integer_ref(&sr.end)?;
                    let end = signed_integer_to_i32(end_si)?;
                    let end_index = ctx.add_i32_constant(end);
                    emitter.emit_load_const_i32(end_index);
                    emit_le(emitter, op_type);
                }
                OpWidth::W64 => {
                    let start_si = resolve_signed_integer_ref(&sr.start)?;
                    let start = signed_integer_to_i64(start_si)?;
                    let start_index = ctx.add_i64_constant(start);
                    emitter.emit_load_const_i64(start_index);
                    emit_ge(emitter, op_type);

                    compile_expr(emitter, ctx, selector_expr, op_type)?;
                    let end_si = resolve_signed_integer_ref(&sr.end)?;
                    let end = signed_integer_to_i64(end_si)?;
                    let end_index = ctx.add_i64_constant(end);
                    emitter.emit_load_const_i64(end_index);
                    emit_le(emitter, op_type);
                }
                // CASE with float types is not meaningful in IEC 61131-3.
                _ => return Err(Diagnostic::todo(file!(), line!())),
            }

            emitter.emit_bool_and();
            Ok(())
        }
        CaseSelectionKind::EnumeratedValue(ev) => {
            // REQ-EN-040: Load selector, load ordinal constant, compare with EQ_I32.
            compile_expr(emitter, ctx, selector_expr, op_type)?;
            let ordinal = crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, ev)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            emitter.emit_eq_i32();
            Ok(())
        }
    }
}

/// Converts a `SignedInteger` AST node to an `i32` value.
/// Extracts the max length from a `StringInitializer`, returning a
/// not-implemented diagnostic if the length is an unresolved constant reference.
pub(crate) fn resolve_string_max_length(
    string_init: &StringInitializer,
) -> Result<u16, Diagnostic> {
    match &string_init.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH_U16),
        Some(IntegerRef::Literal(i)) => Ok(i.value as u16),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id, file!(), line!())),
    }
}

/// Extracts the max length from a `StringSpecification` (used for function
/// return types), returning a not-implemented diagnostic if the length is an
/// unresolved constant reference.
pub(crate) fn resolve_string_spec_max_length(
    spec: &StringSpecification,
) -> Result<u16, Diagnostic> {
    match &spec.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH_U16),
        Some(IntegerRef::Literal(i)) => Ok(i.value as u16),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id, file!(), line!())),
    }
}

/// Extracts a concrete `SignedInteger` from a `SignedIntegerRef`, returning a
/// not-implemented diagnostic if it is an unresolved constant reference.
fn resolve_signed_integer_ref(sir: &SignedIntegerRef) -> Result<&SignedInteger, Diagnostic> {
    match sir {
        SignedIntegerRef::Literal(si) => Ok(si),
        SignedIntegerRef::Constant(id) => Err(Diagnostic::todo_with_id(id, file!(), line!())),
    }
}

pub(crate) fn signed_integer_to_i32(si: &SignedInteger) -> Result<i32, Diagnostic> {
    if si.is_neg {
        let unsigned = si.value.value as i128;
        let signed = -unsigned;
        i32::try_from(signed).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &signed.to_string())
        })
    } else {
        i32::try_from(si.value.value).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &si.value.value.to_string())
        })
    }
}

/// Compiles a WHILE statement.
///
/// ```text
/// LOOP:
///   compile(condition)
///   JMP_IF_NOT → END
///   compile(body)
///   JMP → LOOP
/// END:
/// ```
fn compile_while(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    while_stmt: &ironplc_dsl::textual::While,
) -> Result<(), Diagnostic> {
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    emitter.bind_label(loop_label);
    let cond_type = condition_op_type(&while_stmt.condition)?;
    compile_expr(emitter, ctx, &while_stmt.condition, cond_type)?;
    emitter.emit_jmp_if_not(end_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &while_stmt.body)?;
    ctx.loop_exit_labels.pop();
    emitter.emit_jmp(loop_label);
    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles a REPEAT statement.
///
/// ```text
/// LOOP:
///   compile(body)
///   compile(condition)
///   JMP_IF_NOT → LOOP
/// ```
fn compile_repeat(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    repeat_stmt: &ironplc_dsl::textual::Repeat,
) -> Result<(), Diagnostic> {
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    emitter.bind_label(loop_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &repeat_stmt.body)?;
    ctx.loop_exit_labels.pop();
    let cond_type = condition_op_type(&repeat_stmt.until)?;
    compile_expr(emitter, ctx, &repeat_stmt.until, cond_type)?;
    emitter.emit_jmp_if_not(loop_label);
    emitter.bind_label(end_label);

    Ok(())
}

/// Whether a compile-time constant step is positive or negative.
enum StepSign {
    Positive,
    Negative,
}

/// Inspects an expression and returns its sign if it is a compile-time constant
/// integer literal (positive or negative). Returns `None` for non-constant
/// expressions.
fn try_constant_sign(expr: &Expr) -> Option<StepSign> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            if lit.value.is_neg {
                Some(StepSign::Negative)
            } else {
                Some(StepSign::Positive)
            }
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => {
            // -<literal> is negative
            if matches!(
                &unary.term.kind,
                ExprKind::Const(ConstantKind::IntegerLiteral(_))
            ) {
                Some(StepSign::Negative)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Compiles a FOR statement.
///
/// ```text
///   compile(from)
///   STORE_VAR control
/// LOOP:
///   LOAD_VAR control
///   compile(to)
///   GT_I32 (or LT_I32 for negative step)
///   JMP_IF_NOT → BODY
///   JMP → END
/// BODY:
///   compile(body)
///   LOAD_VAR control
///   compile(step)  // default: LOAD_CONST 1
///   ADD_I32
///   STORE_VAR control
///   JMP → LOOP
/// END:
/// ```
fn compile_for(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    for_stmt: &ironplc_dsl::textual::For,
) -> Result<(), Diagnostic> {
    let var_index = ctx.var_index(&for_stmt.control)?;
    let op_type = ctx.var_op_type(&for_stmt.control);
    let type_info = ctx.var_type_info(&for_stmt.control);

    // Determine step sign.
    let step_sign = match &for_stmt.step {
        None => StepSign::Positive,
        Some(step_expr) => match try_constant_sign(step_expr) {
            Some(sign) => sign,
            None => {
                return Err(Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(
                        for_stmt.control.span(),
                        "FOR loop step must be a constant expression",
                    ),
                ))
            }
        },
    };

    // Initialize: compile(from), STORE_VAR control
    compile_expr(emitter, ctx, &for_stmt.from, op_type)?;
    if let Some(ti) = type_info {
        emit_truncation(emitter, ti);
    }
    emit_store_var(emitter, var_index, op_type);

    let loop_label = emitter.create_label();
    let body_label = emitter.create_label();
    let end_label = emitter.create_label();

    // LOOP: check termination condition
    emitter.bind_label(loop_label);
    emit_load_var(emitter, var_index, op_type);
    compile_expr(emitter, ctx, &for_stmt.to, op_type)?;
    match step_sign {
        StepSign::Positive => emit_gt(emitter, op_type),
        StepSign::Negative => emit_lt(emitter, op_type),
    }
    emitter.emit_jmp_if_not(body_label);
    emitter.emit_jmp(end_label);

    // BODY:
    emitter.bind_label(body_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &for_stmt.body)?;
    ctx.loop_exit_labels.pop();

    // Increment: LOAD_VAR control, compile(step), ADD, truncate, STORE_VAR control
    emit_load_var(emitter, var_index, op_type);
    match &for_stmt.step {
        Some(step_expr) => compile_expr(emitter, ctx, step_expr, op_type)?,
        None => match op_type.0 {
            OpWidth::W32 => {
                let one_index = ctx.add_i32_constant(1);
                emitter.emit_load_const_i32(one_index);
            }
            OpWidth::W64 => {
                let one_index = ctx.add_i64_constant(1);
                emitter.emit_load_const_i64(one_index);
            }
            OpWidth::F32 => {
                let one_index = ctx.add_f32_constant(1.0);
                emitter.emit_load_const_f32(one_index);
            }
            OpWidth::F64 => {
                let one_index = ctx.add_f64_constant(1.0);
                emitter.emit_load_const_f64(one_index);
            }
        },
    }
    emit_add(emitter, op_type);
    if let Some(ti) = type_info {
        emit_truncation(emitter, ti);
    }
    emit_store_var(emitter, var_index, op_type);
    emitter.emit_jmp(loop_label);

    // END:
    emitter.bind_label(end_label);

    Ok(())
}
