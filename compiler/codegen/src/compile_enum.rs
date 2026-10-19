//! Enumeration support for IEC 61131-3 code generation.
//!
//! The analyzer records each enumeration's members, their ordinals and its
//! default with the type (`SemanticType::Enumeration::members`), and
//! gives every enumerated value a type. Codegen only looks an ordinal up by
//! `(TypeId, value)`; it neither walks declarations nor numbers members.
//!
//! See `specs/design/enumeration-codegen.md` for the full design.

use ironplc_analyzer::enumeration_members::EnumerationMembers;
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::debug_section::EnumDefEntry;
use ironplc_dsl::common::EnumeratedValue;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Expr;
use ironplc_dsl::type_id::TypeId;

use super::compile::{CompileContext, OpWidth, Signedness, VarTypeInfo};

/// The ordinal of `value` as a member of the enumeration `members`.
///
/// `None` for `members` means the analyzer gave the value no enumeration
/// type: a value it could not place, such as one of an inline enumeration
/// declared in a structure field (#1946). That is refused as not
/// implemented, as it was before the analyzer typed enumerated values.
pub(crate) fn ordinal_in(
    members: Option<&EnumerationMembers>,
    value: &EnumeratedValue,
) -> Result<i32, Diagnostic> {
    members
        .and_then(|members| members.ordinal_of(&value.value))
        .map(|ordinal| ordinal as i32)
        .ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                value.span(),
                "Enumerated value whose enumeration is not known",
            ))
        })
}

/// The members of the enumeration `id` identifies; `None` when `id` is not
/// an enumeration.
pub(crate) fn members_of(ctx: &CompileContext, id: Option<TypeId>) -> Option<&EnumerationMembers> {
    ctx.types.get(&id?)?.enumeration_members()
}

/// The members of the enumeration `expr`'s value has as its type.
pub(crate) fn members_of_expr<'a>(
    ctx: &'a CompileContext,
    expr: &Expr,
) -> Option<&'a EnumerationMembers> {
    members_of(ctx, expr.expr_type.as_ref()?.type_id())
}

/// The name the debug section knows the enumeration type `id` by: its
/// declared name, upper-cased, or for an anonymous enumeration
/// (`e : (A, B)`) a name made from its id (REQ-EN-codegen-092). Empty when
/// `id` is unknown.
pub(crate) fn debug_name(types: &TypeEnvironment, id: Option<TypeId>) -> String {
    match id {
        Some(id) => match types.name_of(id) {
            Some(name) => name.to_string().to_uppercase(),
            None => anonymous_name(id),
        },
        None => String::new(),
    }
}

/// The debug name of the anonymous enumeration `id`. A parenthesis cannot
/// start an identifier, so the name is never a declared type's.
fn anonymous_name(id: TypeId) -> String {
    format!("(ANONYMOUS ENUMERATION {})", id.raw())
}

/// The ENUM_DEF entries: one per enumeration type, its members' names in
/// declaration order, sorted by type name so one source always gives the
/// same bytes.
pub(crate) fn enum_definitions(types: &TypeEnvironment) -> Vec<EnumDefEntry> {
    let mut entries: Vec<EnumDefEntry> = types
        .iter_ids()
        .filter_map(|(id, attributes)| {
            let members = attributes.representation.enumeration_members()?;
            Some(EnumDefEntry {
                type_name: debug_name(types, Some(id)),
                values: members
                    .iter()
                    .map(|m| m.name.to_string().to_uppercase())
                    .collect(),
            })
        })
        .collect();
    entries.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    entries
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
    fn ordinal_in_when_no_enumeration_then_not_implemented() {
        let result = ordinal_in(None, &EnumeratedValue::new("RED"));

        assert_eq!(result.unwrap_err().code, "P9999");
    }

    #[test]
    fn anonymous_name_when_id_then_cannot_be_identifier() {
        let name = anonymous_name(TypeId::from_raw(42));

        assert_eq!(name, "(ANONYMOUS ENUMERATION 42)");
    }
}
