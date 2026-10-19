//! The debug type tag and type name of a variable, which its debug entry
//! records.

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::debug_section::{iec_type_tag, var_section};
use ironplc_dsl::common::{
    FunctionReturnType, InitialValueAssignmentKind, SpecificationKind, SubrangeSpecificationKind,
    TypeName, VarDecl, VariableType,
};
use ironplc_dsl::type_id::TypeId;

use crate::compile::char_width_for_string_type;

/// Maps a DSL VariableType to the debug section var_section encoding.
pub(crate) fn map_var_section(vt: &VariableType) -> u8 {
    match vt {
        VariableType::Var => var_section::VAR,
        VariableType::VarTemp => var_section::VAR_TEMP,
        VariableType::Input => var_section::VAR_INPUT,
        VariableType::Output => var_section::VAR_OUTPUT,
        VariableType::InOut => var_section::VAR_IN_OUT,
        VariableType::External => var_section::VAR_EXTERNAL,
        VariableType::Global => var_section::VAR_GLOBAL,
        VariableType::Access => var_section::VAR,
    }
}

/// The debug type tag of the type `type_name` names: the type's id when it
/// is elementary, its base type's tag when it is a subrange (or an alias of
/// one), else `OTHER`.
pub(super) fn resolve_iec_type_tag(types: &TypeEnvironment, type_name: &TypeName) -> u8 {
    types
        .id_of(type_name)
        .map(|type_id| iec_type_tag_of(types, type_id))
        .unwrap_or(iec_type_tag::OTHER)
}

/// The debug type tag of the type `type_id`: the type's id when it is
/// elementary, its base type's tag when it is a subrange (or an alias of
/// one), else `OTHER`.
pub(super) fn iec_type_tag_of(types: &TypeEnvironment, type_id: TypeId) -> u8 {
    if let Some(tag) = ironplc_analyzer::type_id::elementary_debug_tag(type_id) {
        return tag;
    }
    match types.get_by_id(type_id).map(|attrs| &attrs.representation) {
        Some(subrange @ SemanticType::Subrange { .. }) => subrange_iec_type_tag(types, subrange),
        _ => iec_type_tag::OTHER,
    }
}

/// The debug type tag of a subrange type: the tag of the elementary type it
/// is a range of (REQ-SR-023). A subrange's slot holds its value at the base
/// type's width and signedness, so it must render as that type -- a `LINT`
/// subrange rendered as 32 bits shows only the low word. `OTHER` when the
/// base type is not elementary.
pub(super) fn subrange_iec_type_tag(types: &TypeEnvironment, subrange: &SemanticType) -> u8 {
    let mut base = subrange;
    while let SemanticType::Subrange { base_type, .. } = base {
        base = base_type;
    }
    types
        .elementary_type_name_for(base)
        .and_then(|name| types.id_of(&name))
        .and_then(ironplc_analyzer::type_id::elementary_debug_tag)
        .unwrap_or(iec_type_tag::OTHER)
}

/// The debug type name of a subrange variable: the declared type's name, or
/// the base type's name for an inline subrange.
fn subrange_debug_type_name(spec: &SubrangeSpecificationKind) -> String {
    match spec {
        SpecificationKind::Named(tn) => tn.to_string().to_uppercase(),
        SpecificationKind::Inline(inline) => format!("{}", inline.type_name),
    }
}

/// Computes the debug `(iec_type_tag, type_name)` pair for a function- or
/// FB-local variable declaration. This mirrors the best-effort resolution
/// the program/global path performs (see `storage::assign_variables`) but
/// without its side-effecting data-region allocation, so it is safe to call
/// from the per-function slot-assignment loops in `compile_fn`. Composite or
/// unsupported initializers fall back to [`iec_type_tag::OTHER`] with a
/// best-effort type name, matching the global behavior.
pub(crate) fn debug_type_for_decl(decl: &VarDecl, types: &TypeEnvironment) -> (u8, String) {
    match &decl.initializer {
        InitialValueAssignmentKind::Simple(simple) => (
            resolve_iec_type_tag(types, &simple.type_name),
            simple.type_name.name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::String(string_init) => {
            if char_width_for_string_type(&string_init.width).is_wide() {
                (iec_type_tag::WSTRING, "WSTRING".into())
            } else {
                (iec_type_tag::STRING, "STRING".into())
            }
        }
        InitialValueAssignmentKind::Reference(_) => (iec_type_tag::OTHER, "REF_TO".into()),
        // A local aggregate keeps its contents in the data region and its slot
        // holds their offset, exactly as a program-level one does.
        InitialValueAssignmentKind::Structure(struct_init) => (
            iec_type_tag::STRUCT,
            struct_init.type_name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::FunctionBlock(fb_init) => (
            iec_type_tag::FB_INSTANCE,
            fb_init.type_name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::EnumeratedType(_)
        | InitialValueAssignmentKind::EnumeratedValues(_) => (
            iec_type_tag::DINT,
            crate::compile_enum::debug_name(types, decl.type_id),
        ),
        InitialValueAssignmentKind::Subrange(spec) => {
            let tag = match &spec.spec {
                SpecificationKind::Named(type_name) => resolve_iec_type_tag(types, type_name),
                SpecificationKind::Inline(inline) => {
                    resolve_iec_type_tag(types, &inline.type_name.clone().into())
                }
            };
            (tag, subrange_debug_type_name(&spec.spec))
        }
        _ => (iec_type_tag::OTHER, String::new()),
    }
}

/// Computes the debug `(iec_type_tag, type_name)` pair for a user
/// function's return variable, derived from its declared return type.
pub(crate) fn debug_type_for_return(
    return_type: &FunctionReturnType,
    types: &TypeEnvironment,
) -> (u8, String) {
    match return_type {
        FunctionReturnType::String(_) => (iec_type_tag::STRING, "STRING".into()),
        FunctionReturnType::WString(_) => (iec_type_tag::WSTRING, "WSTRING".into()),
        FunctionReturnType::Named(_) => {
            let type_name = return_type.to_type_name();
            (
                resolve_iec_type_tag(types, &type_name),
                type_name.name.to_string().to_uppercase(),
            )
        }
    }
}
