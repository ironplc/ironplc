//! Writes to the fields of a function block instance.
//!
//! Writing one field is a fixed four-instruction sequence — load the
//! instance, evaluate the value, store it into the named parameter slot,
//! drop the instance handle — wrapped in a field lookup that resolves the
//! field's name to its slot index and operand type.
//!
//! It lives here rather than inline in either caller because the sequence
//! belongs to the function block instance, not to what happens to be
//! setting the member. Two spellings set one:
//!
//! * an assignment statement (`timer.PT := T#100MS;`), and
//! * a declaration's member initializer (`timer : TON := (PT := T#100MS);`),
//!   emitted once as part of the setup block.
//!
//! One copy of the sequence is what makes those two observably the same
//! thing at runtime.

use crate::initial_value::{InitialValue, ScalarValue};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Expr;

use super::compile::{CompileContext, OpType, DEFAULT_OP_TYPE};
use super::compile_expr::{compile_expr, op_type};
use super::compile_initial_value::{emit_reference, emit_scalar, is_cleared};
use crate::emit::Emitter;

/// Resolves the operand type for a function block field.
///
/// A user-defined function block records the operand type of each of its
/// fields, so its own table answers first. A standard library block records
/// none — the intrinsic owns its layout, not codegen — so its fields take
/// the default slot type.
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
    DEFAULT_OP_TYPE
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
    let Some(type_id) = ctx.fb_instances.get(instance_name).map(|fb| fb.type_id) else {
        return Ok(false);
    };
    let op_type = resolve_fb_field_op_type(ctx, type_id, &field.to_string().to_lowercase());
    compile_fb_field_store_with(emitter, ctx, instance_name, field, |emitter, ctx| {
        compile_expr(emitter, ctx, value, op_type)
    })
}

/// Emits a store into `field` of the function block instance named
/// `instance_name` of the value `push` leaves on the stack. `Ok(false)`, and
/// nothing emitted, when `instance_name` is not a function block instance.
fn compile_fb_field_store_with(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    instance_name: &Id,
    field: &Id,
    push: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<(), Diagnostic>,
) -> Result<bool, Diagnostic> {
    let field_name = field.to_string().to_lowercase();
    let (field_idx, var_index) = match ctx.fb_instances.get(instance_name) {
        Some(fb_info) => {
            let field_idx = fb_info
                .field_indices
                .get(&field_name)
                .copied()
                .ok_or_else(|| {
                    Diagnostic::not_implemented(Label::span(
                        field.span(),
                        format!(
                            "Unknown field '{field}' on function block '{instance_name}' \
                             (writing a PROPERTY is not supported yet)"
                        ),
                    ))
                })?;
            (field_idx, fb_info.var_index)
        }
        None => return Ok(false),
    };

    emitter.emit_fb_load_instance(var_index);
    push(emitter, ctx)?;
    emitter.emit_fb_store_param(field_idx);
    emitter.emit_pop();
    Ok(true)
}

/// Emits the stores that give a function block instance its starting value
/// (`timer : TON := (PT := T#100MS);`): `value` holds every input, output and
/// internal variable of the instance, the block's defaults with the
/// declaration's member initializers applied over them.
///
/// The instance's data region starts cleared, so only a member whose value
/// would not leave it so is stored, exactly as the equivalent assignment
/// statement would store it. The instance is then initialized before the
/// first scan invokes it.
pub(crate) fn emit_fb_instance_member_initializers(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    instance_name: &Id,
    value: &InitialValue,
) -> Result<(), Diagnostic> {
    let InitialValue::Structure(fields) = value else {
        return Err(Diagnostic::internal_error_at(Label::span(
            instance_name.span(),
            "Function block instance value is not a structure of its members",
        )));
    };
    for field in fields {
        if is_cleared(&field.value) {
            continue;
        }
        match &field.value {
            InitialValue::Scalar(scalar) => {
                // The member is stored at the type its value is stored as;
                // a standard block records no types of its own.
                let op_type = scalar_op_type(scalar, &field.name)?;
                compile_fb_field_store_with(emitter, ctx, instance_name, &field.name, |e, c| {
                    emit_scalar(e, c, scalar, op_type, &field.name.span())
                })?;
            }
            InitialValue::Reference(reference) => {
                compile_fb_field_store_with(emitter, ctx, instance_name, &field.name, |e, c| {
                    emit_reference(e, c, reference)
                })?;
            }
            // An expression member (an extension) is evaluated when the
            // instance is initialized, at the member's type in a
            // user-defined block and at its own type in a standard one,
            // which records no member types.
            InitialValue::Expression(expr) => {
                let op_type = match user_field_op_type(ctx, instance_name, &field.name) {
                    Some(op_type) => op_type,
                    None => op_type(ctx, expr)?,
                };
                compile_fb_field_store_with(emitter, ctx, instance_name, &field.name, |e, c| {
                    compile_expr(e, c, expr, op_type)
                })?;
            }
            // A string, array, structure or block member lives outside the
            // member's slot, which the single-slot FB_STORE_PARAM path cannot
            // express. Refuse rather than silently leave the member cleared.
            InitialValue::String(_) | InitialValue::Array(_) | InitialValue::Structure(_) => {
                return Err(Diagnostic::not_implemented(Label::span(
                    field.name.span(),
                    format!(
                        "Array, string or structure value initializing field '{}' of function block instance '{instance_name}'",
                        field.name
                    ),
                )))
            }
        }
    }
    Ok(())
}

/// The operation type of `field` of the instance `instance_name` of a
/// user-defined function block, which records one for each of its fields.
fn user_field_op_type(ctx: &CompileContext, instance_name: &Id, field: &Id) -> Option<OpType> {
    let type_id = ctx.fb_instances.get(instance_name)?.type_id;
    ctx.user_fb_types
        .values()
        .find(|user_fb| user_fb.type_id == type_id)?
        .field_op_types
        .get(&field.to_string().to_lowercase())
        .copied()
}

/// The operation type a scalar member value is stored at: the type of its
/// representation.
fn scalar_op_type(scalar: &ScalarValue, field: &Id) -> Result<OpType, Diagnostic> {
    crate::compile_struct::resolve_field_op_type(&scalar.storage).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            field.span(),
            "Function block member value has no storage type",
        ))
    })
}
