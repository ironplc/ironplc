//! Assignment statement compilation for IEC 61131-3 code generation.
//!
//! Separated from `compile_stmt.rs` to keep module sizes within the
//! 1000-line guideline.

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Assignment, Expr, SymbolicVariableKind, Variable};

use super::compile::{CompileContext, DEFAULT_OP_TYPE};
use super::compile_array::{emit_flat_index, resolve_access, ResolvedAccess};
use super::compile_expr::{compile_expr, emit_truncation, op_type_from_expr, variable_span};
use super::compile_fb_init::compile_fb_field_store;
use super::compile_partial_access::{compile_partial_access_assignment, PartialAccess};
use super::compile_place::Place;
use crate::emit::Emitter;
use crate::storage::Binding;
use crate::string_width::compile_string_value;

/// Compiles an assignment statement.
pub(crate) fn compile_assignment(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    assignment: &Assignment,
) -> Result<(), Diagnostic> {
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
        let target = Binding::of_variable(&assignment.target)?;
        // A VAR_IN_OUT parameter's slot holds a reference to the
        // caller's variable, not the reference to dereference.
        let target_index = target
            .filter(|target| !ctx.in_out_params.contains(&target.decl))
            .and_then(|target| ctx.variables.get(&target.decl).copied())
            .ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    assignment.target.span(),
                    "Dereferenced assignment target is not a plain variable",
                ))
            })?;

        // The analyzer converted the value to the referenced type
        // (REQ-IC-analyzer-039), so it compiles at its own type. It is
        // stored as it is: a result narrower than 32 bits is not truncated to
        // its width (#2116).
        let op_type = op_type_from_expr(ctx, &assignment.value).unwrap_or(DEFAULT_OP_TYPE);
        compile_expr(emitter, ctx, &assignment.value, op_type)?;

        // Load the reference (variable index stored in the ref variable).
        emitter.emit_load_var_i64(target_index);

        // STORE_INDIRECT pops both value and ref.
        emitter.emit_store_indirect();
        return Ok(());
    }

    // A bit or partial access target replaces bits of its base
    // (read-modify-write).
    if let Some(access) = PartialAccess::of(&assignment.target) {
        return compile_partial_access_assignment(emitter, ctx, &access, &assignment.value);
    }

    // Check if the target is a structured variable (struct field write).
    // Excludes `s.arr[i].field := ...`, whose record is an array
    // element rather than a fixed-offset struct field. That shape
    // falls through to the `resolve_access` dispatch below.
    let fixed_offset_field = match &assignment.target {
        Variable::Symbolic(symbolic @ SymbolicVariableKind::Structured(structured))
            if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) =>
        {
            Some((symbolic, structured))
        }
        _ => None,
    };
    if let Some((symbolic, structured)) = fixed_offset_field {
        // Function block instance field write (e.g. `timer.IN := TRUE`).
        // FB instances live in `ctx.fb_instances` rather than
        // `ctx.struct_vars`, and their fields are stored in the data
        // region addressed via FB_STORE_PARAM.
        if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
            if compile_fb_field_store(
                emitter,
                ctx,
                Binding::of(named)?,
                &structured.field,
                &assignment.value,
            )? {
                return Ok(());
            }
        }

        // STRING fields are composite (multi-slot) and handled via the
        // data region, so we intercept before resolve_struct_field_access
        // which only supports single-slot (primitive/enum) fields.
        let (root, slot_offset, field_type) = crate::compile_struct::walk_struct_chain(
            ctx,
            &structured.record,
            &structured.field,
            0,
        )?;
        if let ironplc_analyzer::semantic_type::SemanticType::String { char_width, .. } =
            &field_type
        {
            let char_width = *char_width;
            // `walk_struct_chain` found this structure variable above.
            let struct_info = ctx.struct_vars.get(&root.decl).ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    structured.span(),
                    format!("Variable '{}' is not a structure", root.name),
                ))
            })?;
            let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
            // Produce the RHS at the field's declared encoding, the
            // same as any other string destination (ADR-0034).
            compile_string_value(emitter, ctx, &assignment.value, char_width)?;
            emitter.emit_str_store_var(byte_offset);
            return Ok(());
        }

        let place = Place::resolve(ctx, symbolic)?;
        return assign_to_place(emitter, ctx, &place, &assignment.value);
    }

    // Whole-aggregate assignment (`x := y` where x is an array or a
    // structure). Emits COPY_REGION, which moves the bytes rather
    // than the data-region offset the scalar arm below would copy.
    if crate::compile_aggregate::try_compile_whole_assignment(emitter, ctx, assignment)? {
        return Ok(());
    }

    // Look up the target variable's type info.
    let target = Binding::of_variable(&assignment.target)?;

    // Check if the target is a STRING variable (stored in data region).
    let string_info = target
        .and_then(|target| ctx.string_vars.get(&target.decl))
        .map(|info| (info.data_offset, info.char_width));

    if let Some((data_offset, char_width)) = string_info {
        // String target: produce the RHS as a temp buffer at the
        // target's encoding, then STR_STORE_VAR (ADR-0034).
        compile_string_value(emitter, ctx, &assignment.value, char_width)?;
        emitter.emit_str_store_var(data_offset);
    } else {
        match resolve_access(ctx, &assignment.target)? {
            ResolvedAccess::ArrayElement { info, subscripts } if info.is_string_element => {
                // Copy fields from info (borrows ctx) before using ctx mutably.
                let arr_var_index = info.var_index;
                let arr_desc_index = info.desc_index;
                let element_char_width = info.string_char_width;
                let dim_info = info.dimensions.clone();
                // String array: produce the RHS as a temp buffer at the
                // element's encoding, then the flat index, then
                // STR_STORE_ARRAY_ELEM (ADR-0034).
                compile_string_value(emitter, ctx, &assignment.value, element_char_width)?;
                emit_flat_index(
                    emitter,
                    ctx,
                    &subscripts,
                    &dim_info,
                    &variable_span(&assignment.target),
                )?;
                emitter.emit_str_store_array_elem(arr_var_index, arr_desc_index);
            }
            ResolvedAccess::StructFieldStringArrayElement(element) => {
                // The RHS produces, at the element's encoding, the
                // temp buffer index the store consumes (ADR-0034).
                compile_string_value(emitter, ctx, &assignment.value, element.char_width)?;
                element.emit_base_and_index(emitter, ctx, &variable_span(&assignment.target))?;
                emitter.emit_str_store_array_elem(
                    element.scratch_var_index,
                    element.string_desc_index,
                );
            }
            // `a^[i] := v` where `a` is a STRING array rather than a
            // reference to one. The analyzer accepts the dereference, and
            // this has always compiled to a single-slot store through `a`.
            // A place refuses a STRING element, so this shape keeps its
            // own store until the analyzer rejects the dereference.
            ResolvedAccess::DerefArrayElement { info, subscripts } if info.is_string_element => {
                let element_vti = info.element_var_type_info;
                let ref_var_index = info.var_index;
                let arr_desc_index = info.desc_index;
                let dim_info = info.dimensions.clone();
                let element_op_type = (element_vti.op_width, element_vti.signedness);
                compile_expr(emitter, ctx, &assignment.value, element_op_type)?;
                emit_truncation(emitter, element_vti);
                emit_flat_index(
                    emitter,
                    ctx,
                    &subscripts,
                    &dim_info,
                    &variable_span(&assignment.target),
                )?;
                emitter.emit_store_array_deref(ref_var_index, arr_desc_index);
            }
            access => {
                let place = Place::from_access(
                    ctx,
                    access,
                    target.map(|target| target.decl),
                    variable_span(&assignment.target),
                )?;
                assign_to_place(emitter, ctx, &place, &assignment.value)?;
            }
        }
    }
    Ok(())
}

/// Compiles `value` at the type `place` holds, and stores it there.
fn assign_to_place(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    place: &Place,
    value: &Expr,
) -> Result<(), Diagnostic> {
    compile_expr(emitter, ctx, value, place.op_type())?;
    place.emit_store(emitter, ctx)
}
