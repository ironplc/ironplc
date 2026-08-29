//! Whole-aggregate assignment (`x := y` where both sides are arrays or
//! structures).
//!
//! IEC 61131-3 §7.3.3.1 defines assignment over a "single or multi-element
//! variable" as a value copy — arrays and structures alike. An aggregate
//! variable's slot holds its data-region byte offset, so the scalar
//! load/store path would copy the *offset* and leave the destination aliasing
//! the source. This module emits `COPY_REGION` instead, which moves the
//! bytes.
//!
//! Separate from `compile_stmt.rs` to keep module sizes within the 1000-line
//! guideline.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Assignment, ExprKind, SymbolicVariableKind, Variable};

use super::compile::{CompileContext, DEFAULT_OP_TYPE};
use super::compile_expr::{compile_expr, resolve_variable_name};
use crate::emit::Emitter;
use ironplc_container::VarIndex;

/// One end of a whole-aggregate copy: where its data region starts and which
/// array descriptor gives its size.
struct Region {
    var_index: VarIndex,
    desc_index: u16,
}

/// Compiles `dst := src` when `dst` names a whole array or structure
/// variable.
///
/// Returns `Ok(false)` when the target is not a whole aggregate, leaving the
/// caller's existing dispatch (scalar, array element, struct field, STRING)
/// untouched.
///
/// The emitted sequence is:
///
/// ```text
///     <compile RHS>                              ; source data_offset
///     COPY_REGION dst_var, dst_desc, src_desc
/// ```
///
/// No length is emitted. The VM derives both sizes from the descriptors and
/// traps on a disagreement, so a defect here cannot silently over-copy into a
/// neighbouring variable. Declared-type equality is the analyzer's job
/// (P2037, `rule_assignment_aggregate_type_compat`); this function assumes it
/// and only resolves the two regions.
pub(crate) fn try_compile_whole_assignment(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    assignment: &Assignment,
) -> Result<bool, Diagnostic> {
    // Only a plain named variable is a whole-aggregate target. An element or
    // field access (`x[i] := ...`, `s.f := ...`) is a different operation and
    // belongs to the caller's other arms.
    if !matches!(
        &assignment.target,
        Variable::Symbolic(SymbolicVariableKind::Named(_))
    ) {
        return Ok(false);
    }
    let Some(target_name) = resolve_variable_name(&assignment.target) else {
        return Ok(false);
    };

    let Some(dst) = resolve_region(ctx, target_name)? else {
        return Ok(false);
    };

    let src_desc_index = resolve_source_descriptor(ctx, &assignment.value).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            assignment.value.span(),
            "Only another variable of the same type or a function result can be \
             assigned to a whole array or structure",
        ))
    })?;

    // Leaves the source's data_offset on the stack: a bare aggregate variable
    // compiles to a LOAD_VAR of its slot, and a struct-returning call leaves
    // its return region's offset there.
    compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;
    emitter.emit_copy_region(dst.var_index, dst.desc_index, src_desc_index);
    Ok(true)
}

/// Resolves a variable name to the region it occupies, or `None` when it is
/// not a whole aggregate.
fn resolve_region(ctx: &CompileContext, name: &Id) -> Result<Option<Region>, Diagnostic> {
    if let Some(info) = ctx.struct_vars.get(name) {
        return Ok(Some(Region {
            var_index: info.var_index,
            desc_index: info.desc_index,
        }));
    }
    if let Some(info) = ctx.array_vars.get(name) {
        // A `REF_TO ARRAY` parameter's slot holds the target's variable index
        // rather than a data-region offset, and it owns no region. Assigning
        // one is a reference copy, which the scalar path already does
        // correctly, so fall through rather than emitting a region copy of
        // something that is not a region.
        if info.is_ref {
            return Ok(None);
        }
        return Ok(Some(Region {
            var_index: info.var_index,
            desc_index: info.desc_index,
        }));
    }
    Ok(None)
}

/// Resolves the array descriptor that sizes the right-hand side.
///
/// Handles the two shapes that can produce a whole aggregate: another
/// aggregate variable, and a call to a struct-returning user function.
fn resolve_source_descriptor(
    ctx: &CompileContext,
    value: &ironplc_dsl::textual::Expr,
) -> Option<u16> {
    match &value.kind {
        ExprKind::Variable(variable) => {
            let name = resolve_variable_name(variable)?;
            resolve_region(ctx, name).ok().flatten().map(|r| r.desc_index)
        }
        ExprKind::Function(function) => ctx
            .user_functions
            .get(&function.name.to_string().to_lowercase())
            .and_then(|info| info.return_struct_desc_index),
        _ => None,
    }
}
