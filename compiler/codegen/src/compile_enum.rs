//! Enumeration support for IEC 61131-3 code generation.
//!
//! The analyzer records each enumeration's members, their ordinals and its
//! default with the type (`SemanticType::Enumeration::members`), and lowering
//! gives each enumerated value its ordinal. Codegen operates on every
//! enumeration as a `DINT`, and names each in the debug section.
//!
//! See `specs/design/enumeration-codegen.md` for the full design.

use ironplc_analyzer::TypeEnvironment;
use ironplc_dsl::type_id::TypeId;

use super::compile::{OpWidth, Signedness, VarTypeInfo};

/// The name the debug section knows the enumeration type `id` by: its
/// declared name, upper-cased, or for an anonymous enumeration
/// (`e : (A, B)`) a name made from its id (REQ-EN-codegen-092). Empty when
/// `id` is unknown.
pub(crate) fn debug_name(types: &TypeEnvironment, id: Option<TypeId>) -> String {
    match id {
        Some(id) => match types.name_of(id) {
            Some(name) => name.to_string().to_uppercase(),
            None => anonymous_name(id.raw()),
        },
        None => String::new(),
    }
}

/// How the debug name of an anonymous enumeration begins; its id and a
/// parenthesis end it.
pub(crate) const ANONYMOUS_PREFIX: &str = "(ANONYMOUS ENUMERATION ";

/// The debug name of the anonymous enumeration whose id is `id` in the type
/// table the debug section is built from. A parenthesis cannot start an
/// identifier, so the name is never a declared type's.
pub(crate) fn anonymous_name(id: impl std::fmt::Display) -> String {
    format!("{ANONYMOUS_PREFIX}{id})")
}

/// Returns the `VarTypeInfo` for an enumeration variable.
///
/// All enumerations use DINT (W32, Signed, 32-bit) at the codegen level,
/// regardless of the analyzer's underlying type sizing (B8/B16). This avoids
/// unnecessary truncation opcodes since every VM slot is 64 bits wide.
pub(crate) fn enum_var_type_info() -> VarTypeInfo {
    VarTypeInfo {
        op_width: OpWidth::W32,
        signedness: Signedness::Signed,
        storage_bits: 32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_var_type_info_when_called_then_returns_dint() {
        let info = enum_var_type_info();

        assert_eq!(info.op_width, OpWidth::W32);
        assert_eq!(info.signedness, Signedness::Signed);
        assert_eq!(info.storage_bits, 32);
    }

    #[test]
    fn anonymous_name_when_id_then_cannot_be_identifier() {
        let name = anonymous_name(42);

        assert_eq!(name, "(ANONYMOUS ENUMERATION 42)");
    }
}
