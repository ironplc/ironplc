//! The conversion of a value from one operation type to another, which an
//! `ImplicitConversion` the analyzer recorded asks for (ADR-0056).

use ironplc_ir::Diagnostic;

use super::compile::{OpType, VarTypeInfo};
use super::compile_call::emit_conversion_opcode;
use crate::emit::Emitter;

/// Emits the conversion of the value on the stack from `from` to `to`, or
/// nothing when the two share an operation width.
pub(crate) fn convert(emitter: &mut Emitter, from: OpType, to: OpType) -> Result<(), Diagnostic> {
    let info = |(op_width, signedness): OpType| VarTypeInfo {
        op_width,
        signedness,
        storage_bits: 0,
    };
    emit_conversion_opcode(emitter, &info(from), &info(to))
}
