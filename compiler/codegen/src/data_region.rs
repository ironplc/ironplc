//! Reservation of space in the container's data region.
//!
//! Every variable that does not fit in a single variable slot -- an array, a
//! structure, a STRING -- occupies a run of bytes in the data region, and the
//! variable slot holds the offset of the start of that run. This module owns
//! the one function that hands out those runs, so that the limits of the
//! region are enforced identically wherever a variable is allocated.

use ironplc_dsl::{
    core::SourceSpan,
    diagnostic::{Diagnostic, Label},
};

use crate::compile::CompileContext;

/// Reserves `total_bytes` of data region space and returns the offset of the
/// start of the reserved run.
///
/// The two limits enforced here are fixed properties of the bytecode format
/// rather than features awaiting work, so both report P9997 (`NotSupported`)
/// and not P9999 (`NotImplemented`): the running offset is a `u32`, and the
/// offset a variable slot carries is emitted by `LOAD_CONST_I32`, which caps
/// the addressable region at `i32::MAX` (2 GiB).
///
/// The ceiling applies to the *end* of the run, because the last byte of the
/// run is what has to remain addressable.
///
/// `#[track_caller]` keeps the compiler `file#Lline` that
/// [`Diagnostic::not_supported`] records pointing at the caller rather than at
/// this function, so the P9xxx dashboards still rank by the allocation site
/// that reached the limit.
#[track_caller]
pub(crate) fn reserve(
    ctx: &mut CompileContext,
    total_bytes: u32,
    span: &SourceSpan,
) -> Result<u32, Diagnostic> {
    let data_offset = ctx.data_region_offset;

    ctx.data_region_offset = ctx
        .data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| {
            Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow"))
        })?;

    if ctx.data_region_offset > i32::MAX as u32 {
        return Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Data region exceeds 2 GiB limit",
        )));
    }

    Ok(data_offset)
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::SourceSpan;

    use super::reserve;
    use crate::compile::CompileContext;

    #[test]
    fn reserve_when_space_available_then_returns_start_and_advances_offset() {
        let mut ctx = CompileContext::new();
        ctx.data_region_offset = 16;

        let offset = reserve(&mut ctx, 8, &SourceSpan::default()).unwrap();

        assert_eq!(16, offset);
        assert_eq!(24, ctx.data_region_offset);
    }

    #[test]
    fn reserve_when_run_ends_at_i32_max_then_ok() {
        let mut ctx = CompileContext::new();
        ctx.data_region_offset = i32::MAX as u32 - 8;

        let offset = reserve(&mut ctx, 8, &SourceSpan::default()).unwrap();

        assert_eq!(i32::MAX as u32 - 8, offset);
        assert_eq!(i32::MAX as u32, ctx.data_region_offset);
    }

    #[test]
    fn reserve_when_run_ends_past_i32_max_then_not_supported() {
        let mut ctx = CompileContext::new();
        ctx.data_region_offset = i32::MAX as u32 - 8;

        let err = reserve(&mut ctx, 9, &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }

    #[test]
    fn reserve_when_u32_overflows_then_not_supported() {
        let mut ctx = CompileContext::new();
        ctx.data_region_offset = u32::MAX - 8;

        let err = reserve(&mut ctx, 9, &SourceSpan::default()).unwrap_err();

        assert_eq!("P9997", err.code);
    }
}
