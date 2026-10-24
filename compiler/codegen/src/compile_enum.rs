//! Enumeration support for IEC 61131-3 code generation.
//!
//! The analyzer records each enumeration's members, their ordinals and its
//! default with the type (`SemanticType::Enumeration::members`), and lowering
//! gives each enumerated value its ordinal. Codegen operates on every
//! enumeration as a `DINT`, and names each in the debug section.
//!
//! See `specs/design/enumeration-codegen.md` for the full design.

/// How the debug name of an anonymous enumeration begins; its id and a
/// parenthesis end it.
const ANONYMOUS_PREFIX: &str = "(ANONYMOUS ENUMERATION ";

/// The debug name of the anonymous enumeration whose id is `id` in the type
/// table the debug section is built from. A parenthesis cannot start an
/// identifier, so the name is never a declared type's.
pub(crate) fn anonymous_name(id: impl std::fmt::Display) -> String {
    format!("{ANONYMOUS_PREFIX}{id})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_name_when_id_then_cannot_be_identifier() {
        let name = anonymous_name(42);

        assert_eq!(name, "(ANONYMOUS ENUMERATION 42)");
    }
}
