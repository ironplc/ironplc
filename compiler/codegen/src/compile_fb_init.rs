//! The starting values of a function block instance's members.
//!
//! Two spellings set a member:
//!
//! * an assignment statement (`timer.PT := T#100MS;`), and
//! * a declaration's member initializer (`timer : TON := (PT := T#100MS);`),
//!   emitted once as part of the setup block.
//!
//! Both store through the [`Place`] of the field (`compile_place.rs`), so one
//! copy of the store sequence makes the two observably the same thing at
//! runtime.

use crate::initial_value::{InitialValue, ScalarValue};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use super::compile::{CompileContext, OpType};
use super::compile_expr::compile_expr;
use super::compile_initial_value::{emit_reference, emit_scalar, is_cleared};
use super::compile_place::Place;
use crate::emit::Emitter;

/// Emits a store into `field` of the function block instance named
/// `instance_name` of the value `push` leaves on the stack, through the
/// [`Place`] an assignment to the field stores through. Nothing is emitted
/// when `instance_name` is not a function block instance.
fn store_member(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    instance_name: &Id,
    field: &Id,
    push: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match Place::instance_field(ctx, instance_name, field)? {
        Some(place) => place.emit_assign(emitter, ctx, push),
        None => Ok(()),
    }
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
                // The member is stored at its declared type, as an
                // assignment to it is, and at the type its value is stored
                // as when the block records none.
                let op_type = match field_op_type(ctx, instance_name, &field.name) {
                    Some(op_type) => op_type,
                    None => scalar_op_type(scalar, &field.name)?,
                };
                store_member(emitter, ctx, instance_name, &field.name, |e, c| {
                    emit_scalar(e, c, scalar, op_type, &field.name.span())
                })?;
            }
            InitialValue::Reference(reference) => {
                store_member(emitter, ctx, instance_name, &field.name, |e, c| {
                    emit_reference(e, c, reference)
                })?;
            }
            // An expression member (an extension) is evaluated when the
            // instance is initialized, at its own type: the analyzer records
            // any conversion to the member's type.
            InitialValue::Expression(expr) => {
                store_member(emitter, ctx, instance_name, &field.name, |e, c| {
                    compile_expr(e, c, expr)
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

/// The operation type of `field` of the instance `instance_name`, when its
/// block records one for the field.
fn field_op_type(ctx: &CompileContext, instance_name: &Id, field: &Id) -> Option<OpType> {
    ctx.fb_instances
        .get(instance_name)?
        .field_types
        .get(&field.to_string().to_lowercase())
        .map(|field_type| (field_type.op_width, field_type.signedness))
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
