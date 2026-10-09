//! Stores the starting values the analyzer resolved.
//!
//! The analyzer completes every declaration's initializer with the value it
//! starts with, already defaulted, expanded and converted to the type it is
//! stored as (see `specs/design/initial-values.md`), and
//! [`crate::initial_value`] reads it. Codegen decides only where a value is
//! stored and which stores to emit; it decides no value.
//!
//! Which stores to emit is a storage decision. The variable table and the
//! data region start cleared, so a variable set once, when the program
//! starts, need not be stored a value that leaves its storage cleared. A
//! variable set again on every call must be stored whatever its value.

use crate::initial_value::{InitialValue, ReferenceValue, Scalar, ScalarValue, StringValue};
use ironplc_dsl::common::VarDecl;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use super::compile::{emit_string_literal_load, CompileContext, OpType, OpWidth};
use super::compile_expr::resolve_variable;
use super::compile_setup::emit_zero_const;
use crate::emit::Emitter;

/// The value `decl` starts with, read from the initializer the analyzer
/// completed. A declaration whose initializer reaches codegen without a
/// complete value is a compiler defect, reported as an internal error.
pub(crate) fn required(decl: &VarDecl, ctx: &CompileContext) -> Result<InitialValue, Diagnostic> {
    crate::initial_value::read(decl, &ctx.types, &ctx.block_members)
}

/// Whether storing `value` would leave its storage as it starts: cleared.
/// A `NULL` reference is not, because a reference stores `NULL` as all ones.
pub(crate) fn is_cleared(value: &InitialValue) -> bool {
    match value {
        InitialValue::Scalar(scalar) => scalar.value.is_zero(),
        InitialValue::String(string) => string.chars.is_empty(),
        InitialValue::Array(elements) => elements.iter().all(is_cleared),
        InitialValue::Structure(fields) => fields.iter().all(|field| is_cleared(&field.value)),
        InitialValue::Reference(_) | InitialValue::Expression(_) => false,
    }
}

/// Pushes `value` at `op_type`, the operation type of the storage it goes
/// into.
///
/// `FALSE` and every zero are pushed as the zero of the storage width, which
/// is how storage is cleared.
pub(crate) fn emit_scalar(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    value: &ScalarValue,
    op_type: OpType,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if value.value.is_zero() {
        emit_zero_const(emitter, ctx, op_type);
        return Ok(());
    }
    match (value.value, op_type.0) {
        (Scalar::Bool(_), OpWidth::W32) => emitter.emit_load_true(),
        (Scalar::Integer(integer), OpWidth::W32) => {
            let pool_index = ctx.add_i32_constant(integer as i32);
            emitter.emit_load_const_i32(pool_index);
        }
        (Scalar::Integer(integer), OpWidth::W64) => {
            let pool_index = ctx.add_i64_constant(integer as i64);
            emitter.emit_load_const_i64(pool_index);
        }
        (Scalar::Real32(real), OpWidth::F32) => {
            let pool_index = ctx.add_f32_constant(real);
            emitter.emit_load_const_f32(pool_index);
        }
        (Scalar::Real64(real), OpWidth::F64) => {
            let pool_index = ctx.add_f64_constant(real);
            emitter.emit_load_const_f64(pool_index);
        }
        _ => {
            return Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                "Initial value of a type its storage does not hold",
            )))
        }
    }
    Ok(())
}

/// Pushes the 64-bit variable-table index a reference stores: the index of
/// the variable it refers to, or all ones for `NULL`.
pub(crate) fn emit_reference(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    value: &ReferenceValue,
) -> Result<(), Diagnostic> {
    let index: i64 = match value {
        ReferenceValue::To(variable) => resolve_variable(ctx, variable)?.into(),
        ReferenceValue::Null => u64::MAX as i64,
    };
    let pool_index = ctx.add_i64_constant(index);
    emitter.emit_load_const_i64(pool_index);
    Ok(())
}

/// Stores the characters of `value` into the string whose header is at
/// `byte_offset` in the data region. Nothing is emitted for an empty string,
/// which writing the header leaves.
pub(crate) fn emit_string_store(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    value: &StringValue,
    byte_offset: u32,
) {
    if value.chars.is_empty() {
        return;
    }
    emit_string_literal_load(emitter, ctx, &value.chars, char_width(value));
    emitter.emit_str_store_var(byte_offset);
}

/// The per-code-unit width `value` is stored at.
pub(crate) fn char_width(value: &StringValue) -> ironplc_container::CharWidth {
    super::compile::char_width_for_string_type(&value.width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::initial_value::FieldValue;
    use ironplc_analyzer::semantic_type::ByteSized;
    use ironplc_analyzer::SemanticType;

    fn integer(value: i128) -> InitialValue {
        InitialValue::Scalar(ScalarValue {
            storage: SemanticType::Int {
                size: ByteSized::B32,
            },
            value: Scalar::Integer(value),
        })
    }

    #[test]
    fn is_cleared_when_null_reference_then_false() {
        assert!(!is_cleared(&InitialValue::Reference(ReferenceValue::Null)));
    }

    #[test]
    fn is_cleared_when_structure_of_zeros_then_true() {
        let value = InitialValue::Structure(vec![FieldValue {
            name: ironplc_dsl::core::Id::from("a"),
            value: InitialValue::Array(vec![integer(0), integer(0)]),
        }]);

        assert!(is_cleared(&value));
    }

    #[test]
    fn is_cleared_when_one_nonzero_element_then_false() {
        assert!(!is_cleared(&InitialValue::Array(vec![
            integer(0),
            integer(2)
        ])));
    }
}
