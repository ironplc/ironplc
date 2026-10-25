//! A subscript of an array access.
//!
//! The flat index of an array element is computed from its subscripts in one
//! way (`emit_flat_index`). [`Subscript`] is what that computation needs of
//! one subscript of the lowered program.

use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

use ironplc_container::opcode;

use super::compile::{CompileContext, OpType, OpWidth, Signedness};
use crate::emit::Emitter;

/// One subscript of an array access.
pub(crate) trait Subscript {
    /// The subscript's value when it is an integer literal, so the flat
    /// index can be computed at compile time.
    fn literal(&self) -> Option<i32>;

    /// Pushes the subscript's value, a `DINT`.
    fn emit(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic>;
}

/// Emits the check of an array subscript of the type `from`, on the stack,
/// that leaves it a `DINT` (`to`), or traps (V4007) when it is outside the
/// range of `DINT`. The analyzer records such a check, a checked conversion,
/// for a subscript whose type holds values a `DINT` does not
/// (REQ-LOW-analyzer-162); a check from any other type is a defect.
pub(crate) fn emit_subscript_check(
    emitter: &mut Emitter,
    from: OpType,
    to: OpType,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let func_id = subscript_check(from, to).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            span.clone(),
            format!("A checked conversion from {from:?} to {to:?}, not of a subscript"),
        ))
    })?;
    emitter.emit_builtin(func_id)
}

/// The builtin that checks an array subscript of the type `from` and leaves
/// it a `DINT` (`to`); `None` when `from` holds no value outside the range of
/// `DINT`, or `to` is not `DINT`.
pub(crate) fn subscript_check(from: OpType, to: OpType) -> Option<u16> {
    if to != (OpWidth::W32, Signedness::Signed) {
        return None;
    }
    match from {
        (OpWidth::W64, Signedness::Signed) => Some(opcode::builtin::CHECK_SUBSCRIPT_I64),
        (OpWidth::W64, Signedness::Unsigned) => Some(opcode::builtin::CHECK_SUBSCRIPT_U64),
        (OpWidth::W32, Signedness::Unsigned) => Some(opcode::builtin::CHECK_SUBSCRIPT_U32),
        (OpWidth::W32, Signedness::Signed) | (OpWidth::F32 | OpWidth::F64, _) => None,
    }
}
