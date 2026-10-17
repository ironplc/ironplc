//! Emits lowered strings.
//!
//! A string lives in the data region. Its value is computed into one of the
//! VM's temporary buffers (ADR-0052): a literal by `LOAD_CONST_STR`, the
//! string at a place by `STR_LOAD_VAR` or `STR_LOAD_ARRAY_ELEM`, and a string
//! function's result by its opcode. `STR_STORE_VAR` and
//! `STR_STORE_ARRAY_ELEM` copy a buffer into a place, keeping the code units
//! its capacity holds (REQ-LOW-codegen-056). A string function's instruction
//! takes each string as the data region offset of a slot that holds it.
//! While both routes exist, a string place is found where this backend's
//! layout stores it, as the AST route finds it
//! ([`StringPlace`](crate::compile_string_place::StringPlace)).

use ironplc_container::CharWidth;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{Encoding, Place as LoweredPlace, StrExpr, StrExprKind};
use ironplc_lowering::Lowering;

use super::expr::{emit_intrinsic_call, in_place_not_emitted, instruction_of};
use super::place::{resolve_string, variable_name};
use super::{args_can_emit, string_result, subscripts_can_emit};
use crate::compile::{emit_string_literal_load, CompileContext, StringVarInfo};
use crate::compile_string::allocate_string_temp;
use crate::emit::Emitter;

/// Stores the string `value` at `place`.
pub(super) fn emit_assign_str(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    place: &LoweredPlace,
    value: &StrExpr,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    emit_str_expr(emitter, ctx, lowering, value)?;
    resolve_string(lowering, ctx, place, span)?.emit_store(emitter, ctx)
}

/// Whether this backend reaches every string place `value` reads.
pub(super) fn str_can_emit(lowering: &Lowering<'_>, ctx: &CompileContext, value: &StrExpr) -> bool {
    match value.kind() {
        StrExprKind::Literal(_) => true,
        // The result of a moved call is the buffer the call leaves, emitted
        // where it is read (`string_result.rs`).
        StrExprKind::Read(place) => {
            string_result::is_result(ctx, place) || reaches_string(lowering, ctx, place)
        }
        StrExprKind::Call { callee, args } => {
            instruction_of(*callee, args.len()).is_some() && args_can_emit(lowering, ctx, args)
        }
        StrExprKind::InPlaceCall { .. } => false,
    }
}

/// Whether this backend reaches the string `place`, and every subscript on
/// the way to it.
pub(super) fn reaches_string(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> bool {
    subscripts_can_emit(lowering, ctx, place)
        && resolve_string(lowering, ctx, place, &SourceSpan::default()).is_ok()
}

/// The string variable `place` is, the string field of the current
/// instance, or the string output of the function or method being compiled,
/// which this backend keeps in a slot of its own, by name. `None` for any
/// other place.
fn string_storage<'c>(
    lowering: &Lowering<'_>,
    ctx: &'c CompileContext,
    place: &LoweredPlace,
) -> Option<&'c StringVarInfo> {
    ctx.string_vars.get(&variable_name(lowering, ctx, place)?)
}

/// The data region offset of a slot that holds the string `value`, as the
/// AST route's `resolve_string_arg` gives it: the place's own slot for the
/// string at a place, and otherwise a temporary slot of `value`'s encoding
/// and capacity that `value` is computed into.
pub(super) fn str_arg_offset(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
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
fn emit_str_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    value: &StrExpr,
) -> Result<(), Diagnostic> {
    match value.kind() {
        StrExprKind::Literal(chars) => {
            emit_string_literal_load(emitter, ctx, chars, char_width(value.shape().encoding));
        }
        StrExprKind::Read(place) => {
            if !string_result::emit_at_read(emitter, ctx, lowering, place)? {
                resolve_string(lowering, ctx, place, value.span())?.emit_load(emitter, ctx)?;
            }
        }
        StrExprKind::Call { callee, args } => {
            emit_intrinsic_call(emitter, ctx, lowering, *callee, args, value.span())?
        }
        StrExprKind::InPlaceCall { .. } => return Err(in_place_not_emitted(value.span())),
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
