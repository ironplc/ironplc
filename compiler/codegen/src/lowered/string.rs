//! Emits lowered strings.
//!
//! A string lives in the data region. Its value is computed into one of the
//! VM's temporary buffers (ADR-0052): a literal by `LOAD_CONST_STR`, the
//! string at a place by `STR_LOAD_VAR` or `STR_LOAD_ARRAY_ELEM`, and a string
//! function's result by its opcode. `STR_STORE_VAR` and
//! `STR_STORE_ARRAY_ELEM` copy a buffer into a place, keeping the code units
//! its capacity holds (REQ-LOW-codegen-056). A string function's instruction
//! takes each string as the data region offset of a slot that holds it.
//! A string place is found where this backend's layout stores it
//! ([`StringPlace`](crate::compile_string_place::StringPlace)). A string
//! field of another function block instance is the exception: this backend
//! keeps the index of a temporary buffer in the field's slot (issue 1753),
//! with `FB_STORE_PARAM` and `FB_LOAD_PARAM` (`place::instance_slot`). The
//! string a reference refers to, `rs^` or `rsa^[i]`, is reached through the
//! reference, by `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF`
//! (REQ-LOW-codegen-189).

use ironplc_container::CharWidth;
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{Encoding, Place as LoweredPlace, StrExpr, StrExprKind};
use ironplc_ir::Diagnostic;
use ironplc_ir::SourceSpan;

use super::expr::{emit_intrinsic_call, instruction_of};
use super::fn_call::{emit_in_place, str_in_place_can_emit};
use super::place::{instance_slot, resolve_string, variable_name};
use super::{args_can_emit, subscripts_can_emit};
use crate::compile::{emit_string_literal_load, CompileContext, StringVarInfo};
use crate::compile_string::allocate_string_temp;
use crate::emit::Emitter;

/// Stores the string `value` at `place`.
pub(super) fn emit_assign_str(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    place: &LoweredPlace,
    value: &StrExpr,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if let Some(slot) = instance_slot(lowering, ctx, place) {
        return slot.emit_assign_fitting(emitter, ctx, |emitter, ctx| {
            emit_str_expr(emitter, ctx, lowering, value)
        });
    }
    emit_str_expr(emitter, ctx, lowering, value)?;
    resolve_string(lowering, ctx, place, span)?.emit_store(emitter, ctx)
}

/// Whether this backend reaches every string place `value` reads.
pub(super) fn str_can_emit(lowering: &Program, ctx: &CompileContext, value: &StrExpr) -> bool {
    match value.kind() {
        StrExprKind::Literal(_) => true,
        StrExprKind::Read(place) => reaches_string(lowering, ctx, place),
        StrExprKind::Call { callee, args } => {
            instruction_of(*callee, args.len()).is_some() && args_can_emit(lowering, ctx, args)
        }
        StrExprKind::InPlaceCall { call, .. } => str_in_place_can_emit(lowering, ctx, call),
    }
}

/// Whether this backend reaches the string `place`, and every subscript on
/// the way to it.
pub(super) fn reaches_string(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> bool {
    instance_slot(lowering, ctx, place).is_some()
        || (subscripts_can_emit(lowering, ctx, place)
            && resolve_string(lowering, ctx, place, &SourceSpan::default()).is_ok())
}

/// The string variable `place` is, the string field of the current
/// instance, or the string output of the function or method being compiled,
/// which this backend keeps in a slot of its own. `None` for any other
/// place.
fn string_storage<'c>(
    lowering: &Program,
    ctx: &'c CompileContext,
    place: &LoweredPlace,
) -> Option<&'c StringVarInfo> {
    ctx.string_var(&variable_name(lowering, ctx, place)?)
}

/// The data region offset of a slot that holds the string `value`: the
/// place's own slot for the string at a place, and otherwise a temporary
/// slot of `value`'s encoding and capacity that `value` is computed into.
pub(super) fn str_arg_offset(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    value: &StrExpr,
) -> Result<u32, Diagnostic> {
    if let StrExprKind::Read(place) = value.kind() {
        if let Some(place) = string_storage(lowering, ctx, place) {
            return Ok(place.data_offset);
        }
    }
    let shape = value.shape();
    // A string header records a capacity of at most `u16::MAX` code units
    // (ADR-0035).
    let capacity = u16::try_from(shape.capacity).unwrap_or(u16::MAX);
    let offset = allocate_string_temp(
        emitter,
        ctx,
        char_width(shape.encoding),
        capacity,
        value.span(),
    )?;
    emit_str_expr(emitter, ctx, lowering, value)?;
    emitter.emit_str_store_var(offset);
    Ok(offset)
}

/// Computes `value` into a temporary buffer, leaving the buffer's index on
/// the stack.
pub(super) fn emit_str_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    value: &StrExpr,
) -> Result<(), Diagnostic> {
    match value.kind() {
        StrExprKind::Literal(chars) => {
            emit_string_literal_load(emitter, ctx, chars, char_width(value.shape().encoding));
        }
        StrExprKind::Read(place) => match instance_slot(lowering, ctx, place) {
            Some(slot) => slot.emit_load(emitter, ctx)?,
            None => resolve_string(lowering, ctx, place, value.span())?.emit_load(emitter, ctx)?,
        },
        StrExprKind::Call { callee, args } => {
            emit_intrinsic_call(emitter, ctx, lowering, *callee, args, value.span())?
        }
        // The function's string, in the buffer `CALL` leaves.
        StrExprKind::InPlaceCall { call, .. } => {
            emit_in_place(emitter, ctx, lowering, call, value.span())?
        }
    }
    Ok(())
}

/// The width of a code unit of `encoding` in this backend's strings.
fn char_width(encoding: Encoding) -> CharWidth {
    match encoding {
        Encoding::Narrow => CharWidth::Narrow,
        Encoding::Wide => CharWidth::Wide,
    }
}
