//! Emits lowered strings.
//!
//! A string lives in the data region. Its value is computed into one of the
//! VM's temporary buffers (ADR-0052): a literal by `LOAD_CONST_STR`, the
//! string at a place by `STR_LOAD_VAR`, and a string function's result by
//! its opcode. `STR_STORE_VAR` copies a buffer into a place, keeping the code
//! units its capacity holds (REQ-LOW-codegen-056). A string function's
//! instruction takes each string as the data region offset of a slot that
//! holds it. While both routes exist, a string place is found where this
//! backend's layout stores it, by name.

use ironplc_container::CharWidth;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{Encoding, Place as LoweredPlace, StrExpr, StrExprKind};
use ironplc_lowering::Lowering;

use super::expr::{emit_intrinsic_call, instruction_of};
use super::{args_can_emit, storage_name};
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
    let place = string_storage(lowering, ctx, place).ok_or_else(|| no_storage(span))?;
    emitter.emit_str_store_var(place.data_offset);
    Ok(())
}

/// Whether this backend reaches every string place `value` reads.
pub(super) fn str_can_emit(lowering: &Lowering<'_>, ctx: &CompileContext, value: &StrExpr) -> bool {
    match value.kind() {
        StrExprKind::Literal(_) => true,
        StrExprKind::Read(place) => string_storage(lowering, ctx, place).is_some(),
        StrExprKind::Call { callee, args } => {
            instruction_of(*callee, args.len()).is_some() && args_can_emit(lowering, ctx, args)
        }
    }
}

/// Where this backend's layout stores the string `place`: a string variable,
/// or a string field of the current instance, by name. `None` for any other
/// place.
pub(super) fn string_storage<'c>(
    lowering: &Lowering<'_>,
    ctx: &'c CompileContext,
    place: &LoweredPlace,
) -> Option<&'c StringVarInfo> {
    ctx.string_vars.get(&storage_name(lowering, place)?)
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
            let place =
                string_storage(lowering, ctx, place).ok_or_else(|| no_storage(value.span()))?;
            emitter.emit_str_load_var(place.data_offset);
        }
        StrExprKind::Call { callee, args } => {
            emit_intrinsic_call(emitter, ctx, lowering, *callee, args, value.span())?
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

/// The internal error for a string place `can_emit` should have left to the
/// AST route.
fn no_storage(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A lowered string place has no string storage",
    ))
}
