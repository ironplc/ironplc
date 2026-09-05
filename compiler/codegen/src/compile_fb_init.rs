//! Writes to the fields of a function block instance.
//!
//! Writing one field is a fixed four-instruction sequence — load the
//! instance, evaluate the value, store it into the named parameter slot,
//! drop the instance handle — wrapped in a field lookup that resolves the
//! field's name to its slot index and operand type.
//!
//! It lives here rather than inline in the one statement that emits it
//! today (`timer.PT := T#100MS;`) because the sequence belongs to the
//! function block instance, not to assignment: anything that sets a member
//! of an instance emits exactly these instructions, and they are only
//! observably the same thing at runtime if there is one copy of them.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Expr;

use super::compile::{CompileContext, OpType, OpWidth, Signedness, DEFAULT_OP_TYPE};
use super::compile_expr::compile_expr;
use crate::emit::Emitter;

/// Resolves the operand type for a stdlib function block field by name.
pub(crate) fn fb_field_op_type(field_name: &str) -> OpType {
    match field_name {
        "in" | "q" => (OpWidth::W32, Signedness::Signed),
        "pt" | "et" => (OpWidth::W32, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    }
}

/// Resolves the operand type for a function block field, preferring the
/// user-defined function block's own field types over the stdlib names.
pub(crate) fn resolve_fb_field_op_type(
    ctx: &CompileContext,
    type_id: u16,
    field_name: &str,
) -> OpType {
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

/// Emits a store of `value` into `field` of the function block instance
/// named `instance_name`.
///
/// Returns `Ok(false)` without emitting anything when `instance_name` is not
/// a function block instance, so a caller that cannot tell the two apart
/// (an assignment target may equally be a structure field) can fall through
/// to its own handling.
pub(crate) fn compile_fb_field_store(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    instance_name: &Id,
    field: &Id,
    value: &Expr,
) -> Result<bool, Diagnostic> {
    let field_name = field.to_string().to_lowercase();
    let (field_idx, var_index, type_id) = match ctx.fb_instances.get(instance_name) {
        Some(fb_info) => {
            let field_idx = fb_info
                .field_indices
                .get(&field_name)
                .copied()
                .ok_or_else(|| {
                    Diagnostic::not_implemented(Label::span(
                        field.span(),
                        format!("Unknown field '{field}' on function block '{instance_name}'"),
                    ))
                })?;
            (field_idx, fb_info.var_index, fb_info.type_id)
        }
        None => return Ok(false),
    };

    let op_type = resolve_fb_field_op_type(ctx, type_id, &field_name);
    emitter.emit_fb_load_instance(var_index);
    compile_expr(emitter, ctx, value, op_type)?;
    emitter.emit_fb_store_param(field_idx);
    emitter.emit_pop();
    Ok(true)
}
