//! Statement and control flow compilation for IEC 61131-3 code generation.
//!
//! Contains statement dispatch, control flow (IF, CASE), and function block
//! call compilation; the loops (FOR, WHILE, REPEAT) are in `compile_loop`.
//! Separated from compile.rs to keep module sizes within the 1000-line
//! guideline.

use ironplc_dsl::common::{
    BitStringLiteral, FunctionBlockBodyKind, Integer, IntegerRef, SignedInteger, SignedIntegerRef,
    StringInitializer, StringSpecification,
};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CaseSelectionKind, Expr, FbCall, ParamAssignmentKind, Statements, StmtKind,
    SymbolicVariableKind, Variable,
};
use ironplc_problems::Problem;

use super::compile::{
    CompileContext, CurrentFunctionReturn, OpType, OpWidth, Signedness, DEFAULT_OP_TYPE,
    DEFAULT_STRING_MAX_LENGTH,
};
use super::compile_expr::{
    compile_bit_access_assignment, compile_expr, compile_partial_access_assignment,
    condition_op_type, emit_classified_cmp_br, emit_eq, emit_ge, emit_le, emit_load_var,
    emit_store_var, emit_truncation, extract_bit_access_target, extract_partial_access_target,
    op_type, resolve_variable, resolve_variable_name, try_classify_cmp, variable_span,
};
use super::compile_fb_init::{compile_fb_field_store, resolve_fb_field_op_type};
use super::compile_loop::{compile_for, compile_repeat, compile_while};
use super::compile_method::compile_method_call_statement;
use crate::emit::Emitter;
use crate::string_width::compile_string_value;

/// Compiles a function block body.
///
/// `pou_span` locates the program organization unit that owns the body. An
/// SFC body carries no span of its own, so a diagnostic about the body kind
/// points at the POU instead.
pub(crate) fn compile_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &FunctionBlockBodyKind,
    pou_span: &SourceSpan,
) -> Result<(), Diagnostic> {
    match body {
        FunctionBlockBodyKind::Statements(statements) => {
            compile_statements(emitter, ctx, statements)
        }
        FunctionBlockBodyKind::Empty => Ok(()),
        FunctionBlockBodyKind::Sfc(_) => Err(Diagnostic::not_implemented(Label::span(
            pou_span.clone(),
            "Sequential function chart body",
        ))),
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

/// Records the statement's source position on the emitter so the
/// subsequent opcode(s) get a line-map entry pointing at this
/// statement.
///
/// Looks up the statement's `FileId` in the SOURCE_FILE_TABLE
/// registry built at the start of compilation. If the file isn't
/// registered (synthetic ASTs, unknown spans) or the registry has no
/// source bytes for it (the [`crate::EmptyLookup`] case), no entry is
/// recorded — there's no useful (line, column) to surface, and a
/// `file_id` without a position would only confuse a debugger.
fn record_statement_position(
    emitter: &mut crate::emit::Emitter,
    ctx: &CompileContext,
    stmt: &StmtKind,
) {
    let span = stmt.span();
    let Some(file_id) = ctx.debug_source_files.get(&span.file_id) else {
        return;
    };
    let Some(bytes) = ctx.debug_source_files.source_bytes(&span.file_id) else {
        return;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let lc = ironplc_dsl::diagnostic::LineColumn::from_offset(text, span.start);
    // 0-based → 1-based; clamp to u16 range (source files larger than
    // ~64k lines are theoretical, but the saturating add keeps us out
    // of UB territory either way).
    let line = u16::try_from(lc.line.saturating_add(1)).unwrap_or(u16::MAX);
    let column = u16::try_from(lc.column.saturating_add(1)).unwrap_or(u16::MAX);
    emitter.set_source_position(
        file_id,
        ironplc_container::SourceLine::new(line),
        ironplc_container::SourceColumn::new(column),
    );
}

/// Compiles a single statement.
fn compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &StmtKind,
) -> Result<(), Diagnostic> {
    record_statement_position(emitter, ctx, stmt);
    match stmt {
        StmtKind::Assignment(assignment) => {
            // TwinCAT/CODESYS `S=`/`R=` set/reset binding: parsed and
            // analyzed like any other BOOL assignment, but codegen has no
            // lowering for "write only when true, otherwise leave
            // unchanged" yet (unlike `ref_bind`, which reuses the ordinary
            // `ExprKind::Ref` value and needs no special case here at all).
            // Refuse explicitly rather than emit the unconditional store a
            // naive fallthrough would produce. See issue #1680.
            if assignment.set_bind || assignment.reset_bind {
                return Err(Diagnostic::todo_with_span(assignment.span()));
            }

            // Dereference assignment: myRef^ := expr
            // Compile the RHS, load the reference variable, emit STORE_INDIRECT.
            if assignment.deref {
                let target_name = resolve_variable_name(&assignment.target);
                // A VAR_IN_OUT parameter's slot holds a reference to the
                // caller's variable, not the reference to dereference.
                let target_index = target_name
                    .filter(|name| !ctx.in_out_params.contains(*name))
                    .and_then(|name| ctx.variables.get(name).copied())
                    .ok_or_else(|| {
                        Diagnostic::not_implemented(Label::span(
                            assignment.target.span(),
                            "Dereferenced assignment target is not a plain variable",
                        ))
                    })?;

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

            // Check if the target is a partial access variable (read-modify-write).
            if let Some(partial_access) = extract_partial_access_target(&assignment.target) {
                return compile_partial_access_assignment(
                    emitter,
                    ctx,
                    partial_access,
                    &assignment.value,
                );
            }

            // Check if the target is a structured variable (struct field write).
            // Excludes `s.arr[i].field := ...`, whose record is an array
            // element rather than a fixed-offset struct field. That shape
            // falls through to the `resolve_access` dispatch below.
            let fixed_offset_field = match &assignment.target {
                Variable::Symbolic(SymbolicVariableKind::Structured(structured))
                    if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) =>
                {
                    Some(structured)
                }
                _ => None,
            };
            if let Some(structured) = fixed_offset_field {
                // Function block instance field write (e.g. `timer.IN := TRUE`).
                // FB instances live in `ctx.fb_instances` rather than
                // `ctx.struct_vars`, and their fields are stored in the data
                // region addressed via FB_STORE_PARAM.
                if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
                    if compile_fb_field_store(
                        emitter,
                        ctx,
                        &named.name,
                        &structured.field,
                        &assignment.value,
                    )? {
                        return Ok(());
                    }
                }

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
                if let ironplc_analyzer::intermediate_type::IntermediateType::String {
                    char_width,
                    ..
                } = &field_type
                {
                    let char_width = *char_width;
                    let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
                        Diagnostic::not_implemented(Label::span(
                            structured.span(),
                            format!("Variable '{}' is not a structure", root_name),
                        ))
                    })?;
                    let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
                    // Produce the RHS at the field's declared encoding, the
                    // same as any other string destination (ADR-0034).
                    compile_string_value(emitter, ctx, &assignment.value, char_width)?;
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

            // Whole-aggregate assignment (`x := y` where x is an array or a
            // structure). Emits COPY_REGION, which moves the bytes rather
            // than the data-region offset the scalar arm below would copy.
            if crate::compile_aggregate::try_compile_whole_assignment(emitter, ctx, assignment)? {
                return Ok(());
            }

            // Look up the target variable's type info.
            let target_name = resolve_variable_name(&assignment.target);

            // Check if the target is a STRING variable (stored in data region).
            let string_info = target_name
                .and_then(|name| ctx.string_vars.get(name))
                .map(|info| (info.data_offset, info.char_width));

            if let Some((data_offset, char_width)) = string_info {
                // String target: produce the RHS as a temp buffer at the
                // target's encoding, then STR_STORE_VAR (ADR-0034).
                compile_string_value(emitter, ctx, &assignment.value, char_width)?;
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
                    crate::compile_array::ResolvedAccess::InOut { ref_slot } => {
                        // Store through the reference into the caller's variable.
                        let type_info = target_name.and_then(|name| ctx.var_type_info(name));
                        let op_type = type_info
                            .map(|ti| (ti.op_width, ti.signedness))
                            .unwrap_or(DEFAULT_OP_TYPE);
                        compile_expr(emitter, ctx, &assignment.value, op_type)?;
                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }
                        emitter.emit_load_var_i64(ref_slot);
                        emitter.emit_store_indirect();
                    }
                    crate::compile_array::ResolvedAccess::ArrayElement { info, subscripts } => {
                        // Copy scalar fields from info (borrows ctx) before using ctx mutably.
                        let element_vti = info.element_var_type_info;
                        let arr_var_index = info.var_index;
                        let arr_desc_index = info.desc_index;
                        let is_string_elem = info.is_string_element;
                        let element_char_width = info.string_char_width;
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
                            // String array: produce the RHS as a temp buffer at
                            // the element's encoding, then the flat index, then
                            // STR_STORE_ARRAY_ELEM (ADR-0034).
                            compile_string_value(
                                emitter,
                                ctx,
                                &assignment.value,
                                element_char_width,
                            )?;
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
                    crate::compile_array::ResolvedAccess::StructFieldStringArrayElement(
                        element,
                    ) => {
                        // The RHS produces, at the element's encoding, the
                        // temp buffer index the store consumes (ADR-0034).
                        compile_string_value(emitter, ctx, &assignment.value, element.char_width)?;
                        element.emit_base_and_index(
                            emitter,
                            ctx,
                            &variable_span(&assignment.target),
                        )?;
                        emitter.emit_str_store_array_elem(
                            element.scratch_var_index,
                            element.string_desc_index,
                        );
                    }
                }
            }
            Ok(())
        }
        StmtKind::FbCall(fb_call) => compile_fb_call(emitter, ctx, fb_call),
        StmtKind::MethodCall(method_call) => {
            compile_method_call_statement(emitter, ctx, method_call)
        }
        StmtKind::If(if_stmt) => compile_if(emitter, ctx, if_stmt),
        StmtKind::Case(case_stmt) => compile_case(emitter, ctx, case_stmt),
        StmtKind::For(for_stmt) => compile_for(emitter, ctx, for_stmt),
        StmtKind::While(while_stmt) => compile_while(emitter, ctx, while_stmt),
        StmtKind::Repeat(repeat_stmt) => compile_repeat(emitter, ctx, repeat_stmt),
        StmtKind::Return => {
            // Inside a function with a return value, an early RETURN must
            // first push the current return-value variable so the caller
            // sees a value on the stack (matching the trailing load+RET
            // emitted at the end of the function body).
            match ctx.current_function_return {
                Some(CurrentFunctionReturn::Scalar { var_index, op_type }) => {
                    emit_load_var(emitter, var_index, op_type);
                    emitter.emit_ret();
                }
                Some(CurrentFunctionReturn::String { data_offset }) => {
                    emitter.emit_str_load_var(data_offset);
                    emitter.emit_ret();
                }
                None => {
                    emitter.emit_ret_void();
                }
            }
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
        StmtKind::Continue(span) => {
            let label = ctx.current_loop_next().ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ContinueOutsideLoop,
                    Label::span(
                        span.clone(),
                        "CONTINUE must be inside a FOR, WHILE, or REPEAT loop",
                    ),
                )
            })?;
            emitter.emit_jmp(label);
            Ok(())
        }
    }
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
        .ok_or_else(|| Diagnostic::todo_with_span(fb_call.span()))?;
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
                .ok_or_else(|| Diagnostic::todo_with_span(input.name.span()))?;
            let op_type = resolve_fb_field_op_type(ctx, type_id, &field_name);
            compile_expr(emitter, ctx, &input.expr, op_type)?;
            emitter.emit_fb_store_param(*field_idx);
        }
    }

    // Call the function block. Record a call-graph edge for user-defined
    // FBs (intrinsic FBs have no PLC body and never recurse into the
    // dispatch loop, so they contribute no frames).
    emitter.emit_fb_call(type_id);
    if let Some(user_fb) = ctx
        .user_fb_types
        .values()
        .find(|info| info.type_id == type_id)
    {
        ctx.record_call_edge(user_fb.function_id);
    }

    // Read output parameters.
    for param in &fb_call.params {
        if let ParamAssignmentKind::Output(output) = param {
            let field_name = output.src.to_string().to_lowercase();
            let field_idx = field_indices
                .get(&field_name)
                .ok_or_else(|| Diagnostic::todo_with_span(output.src.span()))?;
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

/// Compiles a slice of statements.
pub(crate) fn compile_stmts(
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

    // Jump past the then-body if condition is false. When the condition is
    // a fusable `var <cmp> const`, replace LOAD_VAR + LOAD_CONST + CMP +
    // JMP_IF_NOT with a single `CMP_BR_*` using the negated comparison.
    let next_label = emitter.create_label();
    if let Some(classified) = try_classify_cmp(ctx, &if_stmt.expr) {
        emit_classified_cmp_br(emitter, classified, false, next_label);
    } else {
        let cond_type = condition_op_type(ctx, &if_stmt.expr)?;
        compile_expr(emitter, ctx, &if_stmt.expr, cond_type)?;
        emitter.emit_jmp_if_not(next_label);
    }

    // Compile the then-body.
    compile_stmts(emitter, ctx, &if_stmt.body)?;

    // If there are more branches, jump to end.
    if needs_end_label {
        emitter.emit_jmp(end_label.unwrap());
    }

    emitter.bind_label(next_label);

    // Compile ELSIF clauses.
    for elsif in &if_stmt.else_ifs {
        let elsif_next = emitter.create_label();
        if let Some(classified) = try_classify_cmp(ctx, &elsif.expr) {
            emit_classified_cmp_br(emitter, classified, false, elsif_next);
        } else {
            let elsif_op_type = condition_op_type(ctx, &elsif.expr)?;
            compile_expr(emitter, ctx, &elsif.expr, elsif_op_type)?;
            emitter.emit_jmp_if_not(elsif_next);
        }

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
/// The selector is evaluated once, before any label is compared, and its
/// value stays on the stack while the labels are tried. Each label compares a
/// `DUP` of it, so a selector with a side effect (a call to a function that
/// writes a global, say) runs once however many labels the statement has.
/// Each `CaseStatementGroup` is tried in order, like IF/ELSIF/ELSE; a group
/// with several labels branches to its body on the first that matches.
///
/// ```text
///   compile(selector)
///   // For each group, with labels l1 .. ln:
///   DUP; compare(l1); BOOL_NOT; JMP_IF_NOT → body   // l1 .. l(n-1)
///   DUP; compare(ln); JMP_IF_NOT → next_group
/// body:
///   POP
///   compile(statements)
///   JMP → END
/// next_group:
///   // ... next group ...
///   POP
///   compile(ELSE statements)
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
    // since all enums use DINT at codegen level (REQ-EN-codegen-003).
    let selector = CaseSelector {
        expr: &case_stmt.selector,
        op_type: op_type(ctx, &case_stmt.selector).unwrap_or(crate::compile::DEFAULT_OP_TYPE),
    };

    selector.compile(emitter, ctx)?;
    let depth_with_selector = emitter.stack_depth();

    for group in &case_stmt.statement_groups {
        let body_label = emitter.create_label();
        let next_label = emitter.create_label();

        match group.selectors.split_last() {
            Some((last, others)) => {
                for selection in others {
                    emitter.emit_dup();
                    compile_case_selector(emitter, ctx, &selector, selection)?;
                    emitter.emit_bool_not();
                    emitter.emit_jmp_if_not(body_label);
                }
                emitter.emit_dup();
                compile_case_selector(emitter, ctx, &selector, last)?;
                emitter.emit_jmp_if_not(next_label);
            }
            // A group without labels matches nothing.
            None => emitter.emit_jmp(next_label),
        }

        emitter.bind_label(body_label);
        emitter.emit_pop();
        compile_stmts(emitter, ctx, &group.statements)?;
        emitter.emit_jmp(end_label);

        // Reached only by a failed comparison, which leaves the selector on
        // the stack; the straight-line tracking above has already popped it.
        emitter.bind_label(next_label);
        emitter.reset_stack_depth(depth_with_selector);
    }

    // No group matched: drop the selector, then run ELSE (if present).
    emitter.emit_pop();
    compile_stmts(emitter, ctx, &case_stmt.else_body)?;

    emitter.bind_label(end_label);

    Ok(())
}

/// The selector of a `CASE` statement: the expression every label is
/// compared against, and the width the comparison is made at.
struct CaseSelector<'a> {
    expr: &'a Expr,
    op_type: OpType,
}

impl CaseSelector<'_> {
    /// Evaluates the selector, leaving its value on the stack.
    ///
    /// A `CASE` compares its selector against integer labels, so only an
    /// integer width is meaningful; a float-width selector is rejected
    /// against the selector expression.
    fn compile(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
        match self.op_type.0 {
            OpWidth::W32 | OpWidth::W64 => compile_expr(emitter, ctx, self.expr, self.op_type),
            // CASE with float types is not meaningful in IEC 61131-3.
            OpWidth::F32 | OpWidth::F64 => Err(non_integer_case_selector(self.expr)),
        }
    }

    /// Emits `<value on the stack> <cmp> label` at the selector's width,
    /// consuming the value and leaving a boolean result on the stack.
    fn cmp_label(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        label: CaseLabelValue<'_>,
        cmp: fn(&mut Emitter, OpType),
    ) -> Result<(), Diagnostic> {
        match self.op_type.0 {
            OpWidth::W32 => {
                let pool_index = ctx.add_i32_constant(label.to_i32(self.op_type.1)?);
                emitter.emit_load_const_i32(pool_index);
            }
            OpWidth::W64 => {
                let pool_index = ctx.add_i64_constant(label.to_i64(self.op_type.1)?);
                emitter.emit_load_const_i64(pool_index);
            }
            // `compile` rejected a float selector before any label is compared.
            OpWidth::F32 | OpWidth::F64 => {
                return Err(non_integer_case_selector(self.expr));
            }
        }
        cmp(emitter, self.op_type);
        Ok(())
    }
}

/// A `CASE` label's value, whatever radix it was written in.
///
/// A label is a value, not a bit pattern: `16#FFFFFFFF` and `4294967295` are
/// the same label, and both narrow to the selector's width by the value they
/// state. Analysis rejects a label outside the selector's type (P2026), so a
/// value that does not fit here comes from a selector analysis could not
/// type, such as an untyped literal, and is reported the same way.
struct CaseLabelValue<'a> {
    is_neg: bool,
    magnitude: &'a Integer,
}

impl<'a> CaseLabelValue<'a> {
    fn signed(value: &'a SignedInteger) -> Self {
        Self {
            is_neg: value.is_neg,
            magnitude: &value.value,
        }
    }

    fn radix(literal: &'a BitStringLiteral) -> Self {
        Self {
            is_neg: false,
            magnitude: &literal.value,
        }
    }

    /// The label's value at the selector's 32-bit width. An unsigned
    /// selector holds its value's bit pattern in the slot, so a label for one
    /// is stored the same way.
    fn to_i32(&self, signedness: Signedness) -> Result<i32, Diagnostic> {
        let value = self.value()?;
        match signedness {
            Signedness::Signed => i32::try_from(value).ok(),
            Signedness::Unsigned => u32::try_from(value).ok().map(|value| value as i32),
        }
        .ok_or_else(|| self.overflow())
    }

    /// The label's value at the selector's 64-bit width, stored the way
    /// [`Self::to_i32`] stores it.
    fn to_i64(&self, signedness: Signedness) -> Result<i64, Diagnostic> {
        let value = self.value()?;
        match signedness {
            Signedness::Signed => i64::try_from(value).ok(),
            Signedness::Unsigned => u64::try_from(value).ok().map(|value| value as i64),
        }
        .ok_or_else(|| self.overflow())
    }

    fn value(&self) -> Result<i128, Diagnostic> {
        let magnitude = i128::try_from(self.magnitude.value).map_err(|_| self.overflow())?;
        Ok(if self.is_neg { -magnitude } else { magnitude })
    }

    fn overflow(&self) -> Diagnostic {
        let sign = if self.is_neg { "-" } else { "" };
        Diagnostic::problem(
            Problem::ConstantOverflow,
            Label::span(self.magnitude.span(), "CASE label"),
        )
        .with_context("value", &format!("{sign}{}", self.magnitude.value))
    }
}

/// Compares the selector value on top of the stack against one label,
/// consuming the value and leaving a boolean result on the stack.
///
/// - `SignedInteger`: `selector == value`
/// - `Subrange`: `(selector >= start) AND (selector <= end)`
/// - `EnumeratedValue`: `selector == ordinal` (REQ-EN-codegen-040)
/// - `BitStringLiteral`: `selector == value` (same shape as `SignedInteger`)
fn compile_case_selector(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    selector: &CaseSelector,
    selection: &CaseSelectionKind,
) -> Result<(), Diagnostic> {
    match selection {
        CaseSelectionKind::SignedInteger(si) => {
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(si), emit_eq)
        }
        CaseSelectionKind::Subrange(sr) => {
            // The value is compared twice, so keep a copy under the first result.
            emitter.emit_dup();
            let start = resolve_signed_integer_ref(&sr.start)?;
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(start), emit_ge)?;
            emitter.emit_swap();
            let end = resolve_signed_integer_ref(&sr.end)?;
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(end), emit_le)?;
            emitter.emit_bool_and();
            Ok(())
        }
        CaseSelectionKind::EnumeratedValue(ev) => {
            // REQ-EN-codegen-040: compare the selector with the ordinal constant using EQ_I32.
            let ordinal = crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, ev)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            emitter.emit_eq_i32();
            Ok(())
        }
        CaseSelectionKind::BitStringLiteral(lit) => {
            selector.cmp_label(emitter, ctx, CaseLabelValue::radix(lit), emit_eq)
        }
    }
}

/// Builds the internal error for a `CASE` whose selector is not an integer
/// type, pointing at the selector expression.
///
/// Analysis rejects such a selector (P4053) before codegen runs, so reaching
/// this is a broken invariant rather than a missing capability.
#[track_caller]
fn non_integer_case_selector(selector_expr: &Expr) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        selector_expr.span(),
        "CASE selector is not an integer type",
    ))
}

/// Converts a `SignedInteger` AST node to an `i32` value.
/// Extracts the max length from a `StringInitializer`, returning a
/// not-implemented diagnostic if the length is an unresolved constant reference.
pub(crate) fn resolve_string_max_length(
    string_init: &StringInitializer,
) -> Result<u16, Diagnostic> {
    match &string_init.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH),
        Some(IntegerRef::Literal(i)) => string_length_u16(i),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id)),
    }
}

/// Extracts the max length from a `StringSpecification` (used for function
/// return types), returning a not-implemented diagnostic if the length is an
/// unresolved constant reference.
pub(crate) fn resolve_string_spec_max_length(
    spec: &StringSpecification,
) -> Result<u16, Diagnostic> {
    match &spec.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH),
        Some(IntegerRef::Literal(i)) => string_length_u16(i),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id)),
    }
}

/// Converts a declared string length to the `u16` the string header holds.
///
/// The analyzer rejects a length above `u16::MAX` (P2041) before codegen
/// runs, so a failure here is a compiler defect -- the rule missed a
/// declaration site -- and is reported as one rather than wrapped to a
/// capacity the program never wrote.
fn string_length_u16(length: &Integer) -> Result<u16, Diagnostic> {
    u16::try_from(length.value).map_err(|_| {
        Diagnostic::internal_error_at(Label::span(
            length.span.clone(),
            format!(
                "String length {} was not rejected by the analyzer",
                length.value
            ),
        ))
    })
}

/// Extracts a concrete `SignedInteger` from a `SignedIntegerRef`, returning a
/// not-implemented diagnostic if it is an unresolved constant reference.
fn resolve_signed_integer_ref(sir: &SignedIntegerRef) -> Result<&SignedInteger, Diagnostic> {
    match sir {
        SignedIntegerRef::Literal(si) => Ok(si),
        SignedIntegerRef::Constant(id) => Err(Diagnostic::todo_with_id(id)),
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
