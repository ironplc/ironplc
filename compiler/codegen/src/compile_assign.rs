//! Assignment statement compilation for IEC 61131-3 code generation.
//!
//! Separated from `compile_stmt.rs` to keep module sizes within the
//! 1000-line guideline.

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Assignment, Expr, SymbolicVariableKind, Variable};

use super::compile::CompileContext;
use super::compile_array::{emit_flat_index, resolve_access, ResolvedAccess};
use super::compile_expr::{compile_expr, emit_truncation, resolve_variable_name, variable_span};
use super::compile_partial_access::{compile_partial_access_assignment, PartialAccess};
use super::compile_place::Place;
use super::compile_string_place::StringPlace;
use crate::emit::Emitter;
use crate::string_width::compile_string_value;
use crate::type_info::expr_type_info;

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

    // Dereference assignment: `p^ := v`. The analyzer converted the value to
    // the referenced type (REQ-IC-analyzer-039), so it compiles at its own
    // type, and that is the type of the place it is stored in.
    if assignment.deref {
        let place = Place::<&Expr>::through_reference(
            ctx,
            &assignment.target,
            expr_type_info(ctx, &assignment.value),
        )?;
        return assign_to_place(emitter, ctx, &place, &assignment.value);
    }

    compile_store(emitter, ctx, &assignment.target, &assignment.value)
}

/// Compiles `target := value`: `value` at the type the analyzer recorded for
/// it, stored in `target`.
pub(crate) fn compile_store(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    target: &Variable,
    value: &Expr,
) -> Result<(), Diagnostic> {
    // A bit or partial access target replaces bits of its base
    // (read-modify-write).
    if let Some(access) = PartialAccess::of(target) {
        return compile_partial_access_assignment(emitter, ctx, &access, value);
    }

    // Check if the target is a structured variable (struct field write).
    // Excludes `s.arr[i].field := ...`, whose record is an array
    // element rather than a fixed-offset struct field. That shape
    // falls through to the `resolve_access` dispatch below.
    let fixed_offset_field = match target {
        Variable::Symbolic(symbolic @ SymbolicVariableKind::Structured(structured))
            if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) =>
        {
            Some((symbolic, structured))
        }
        _ => None,
    };
    if let Some((symbolic, structured)) = fixed_offset_field {
        // A field of a function block instance (`timer.IN := TRUE`), which
        // lives in the instance's data region rather than in a structure.
        if let Some(place) = Place::fb_field(ctx, structured)? {
            return assign_to_place(emitter, ctx, &place, value);
        }

        // A STRING field lives in the data region, not in one slot.
        if let Some(place) = StringPlace::resolve(ctx, target)? {
            return assign_string(emitter, ctx, &place, value);
        }

        let place = Place::resolve(ctx, symbolic)?;
        return assign_to_place(emitter, ctx, &place, value);
    }

    // Whole-aggregate assignment (`x := y` where x is an array or a
    // structure). Emits COPY_REGION, which moves the bytes rather
    // than the data-region offset the scalar arm below would copy.
    if crate::compile_aggregate::try_compile_whole_assignment(emitter, ctx, target, value)? {
        return Ok(());
    }

    // A STRING variable or array element lives in the data region, not in
    // one slot.
    if let Some(place) = StringPlace::resolve(ctx, target)? {
        return assign_string(emitter, ctx, &place, value);
    }

    match resolve_access(ctx, target)? {
        // `a^[i] := v` where `a` is a STRING array rather than a reference to
        // one. The analyzer accepts the dereference, and this has always
        // compiled to a single-slot store through `a`. A place refuses a
        // STRING element, so this shape keeps its own store until the
        // analyzer rejects the dereference.
        ResolvedAccess::DerefArrayElement { info, subscripts } if info.is_string_element => {
            let element_vti = info.element_var_type_info;
            let ref_var_index = info.var_index;
            let arr_desc_index = info.desc_index;
            let dim_info = info.dimensions.clone();
            compile_expr(emitter, ctx, value)?;
            emit_truncation(emitter, element_vti);
            emit_flat_index(emitter, ctx, &subscripts, &dim_info, &variable_span(target))?;
            emitter.emit_store_array_deref(ref_var_index, arr_desc_index);
        }
        access => {
            let target_name = resolve_variable_name(target);
            let place = Place::from_access(ctx, access, target_name, variable_span(target))?;
            assign_to_place(emitter, ctx, &place, value)?;
        }
    }
    Ok(())
}

/// Stores `value`, produced as a temporary buffer at the encoding of the
/// string `place` holds (ADR-0034), there.
fn assign_string(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    place: &StringPlace<&Expr>,
    value: &Expr,
) -> Result<(), Diagnostic> {
    compile_string_value(emitter, ctx, value, place.char_width())?;
    place.emit_store(emitter, ctx)
}

/// Compiles `value` at the type `place` holds, and stores it there.
fn assign_to_place(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    place: &Place<&Expr>,
    value: &Expr,
) -> Result<(), Diagnostic> {
    place.emit_assign(emitter, ctx, |emitter, ctx| {
        compile_expr(emitter, ctx, value)
    })
}
