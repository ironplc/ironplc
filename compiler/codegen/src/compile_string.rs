//! A temporary slot of the data region for a string value a statement
//! computes, at the capacity and encoding it needs.

use ironplc_container::CharWidth;
use ironplc_ir::Diagnostic;
use ironplc_ir::SourceSpan;

use super::compile::{string_region_size, CompileContext};
use crate::emit::Emitter;

/// Allocates a data region slot for an intermediate string value.
///
/// Returns the slot's data_offset with its header already initialized at
/// `max_length` and `char_width`. A wide slot also marks the program as
/// holding a wide string so temp buffers are sized in wide bytes (ADR-0035)
/// even when no WSTRING variable is declared.
pub(crate) fn allocate_string_temp(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    char_width: CharWidth,
    max_length: u16,
    func_span: &SourceSpan,
) -> Result<u32, Diagnostic> {
    let data_offset = ctx.data_region_offset;
    let total_bytes = string_region_size(max_length, char_width);
    ctx.data_region_offset = ctx
        .data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| Diagnostic::todo_with_span(func_span.clone()))?;

    if max_length > ctx.max_string_capacity {
        ctx.max_string_capacity = max_length;
    }
    if char_width.is_wide() {
        ctx.has_wide_string = true;
    }

    emitter.emit_str_init(data_offset, max_length, char_width);

    Ok(data_offset)
}
