//! Enumeration support for IEC 61131-3 code generation.
//!
//! Builds an ordinal map from enumeration type declarations and provides
//! helpers to resolve enumeration values to their integer ordinals.
//!
//! See `specs/design/enumeration-codegen.md` for the full design.

use std::collections::HashMap;

use ironplc_dsl::common::{
    DataTypeDeclarationKind, EnumeratedValue, Library, LibraryElementKind, SpecificationKind,
};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use super::compile::{OpWidth, Signedness, VarTypeInfo};

/// Pre-computed ordinal mappings for all named enumeration types.
///
/// Built once at codegen entry from the library's type declarations
/// and stored in `CompileContext` for use by all codegen phases.
#[derive(Default)]
pub(crate) struct EnumOrdinalMap {
    /// Maps (type_name_upper, value_name_upper) → 0-based ordinal.
    ordinals: HashMap<(String, String), i32>,

    /// Maps unqualified value_name_upper → (type_name_upper, ordinal).
    /// The analyzer guarantees unqualified names are unambiguous within scope.
    value_lookup: HashMap<String, (String, i32)>,

    /// Maps type_name_upper → default ordinal (from the type declaration's
    /// default value, or 0 if no default is specified).
    defaults: HashMap<String, i32>,

    /// Maps type_name_upper → ordered list of value names (for debug output).
    #[allow(dead_code)] // Used in PR 5 (enum definition table in debug section).
    pub(crate) definitions: HashMap<String, Vec<String>>,
}

/// Builds the ordinal map by walking enumeration type declarations in the AST.
///
/// For each `TYPE X : (A, B, C) := A; END_TYPE`, records:
/// - ordinals: (X, A)→0, (X, B)→1, (X, C)→2
/// - value_lookup: A→(X, 0), B→(X, 1), C→(X, 2)
/// - defaults: X→0 (ordinal of A)
/// - definitions: X→[A, B, C]
pub(crate) fn build_enum_ordinal_map(library: &Library) -> EnumOrdinalMap {
    let mut ordinals = HashMap::new();
    let mut value_lookup = HashMap::new();
    let mut defaults = HashMap::new();
    let mut definitions = HashMap::new();

    for element in &library.elements {
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(decl)) =
            element
        {
            let type_upper = decl.type_name.to_string().to_uppercase();

            if let SpecificationKind::Inline(spec_values) = &decl.spec_init.spec {
                let mut value_names = Vec::new();

                for (ordinal, ev) in spec_values.values.iter().enumerate() {
                    let val_upper = ev.value.to_string().to_uppercase();
                    ordinals.insert((type_upper.clone(), val_upper.clone()), ordinal as i32);
                    value_lookup.insert(val_upper.clone(), (type_upper.clone(), ordinal as i32));
                    value_names.push(val_upper);
                }

                // Resolve default ordinal from the type declaration.
                let default_ordinal = decl
                    .spec_init
                    .default
                    .as_ref()
                    .and_then(|ev| {
                        let val_upper = ev.value.to_string().to_uppercase();
                        ordinals.get(&(type_upper.clone(), val_upper)).copied()
                    })
                    .unwrap_or(0);

                defaults.insert(type_upper.clone(), default_ordinal);
                definitions.insert(type_upper, value_names);
            }
        }
    }

    EnumOrdinalMap {
        ordinals,
        value_lookup,
        defaults,
        definitions,
    }
}

/// Resolves an `EnumeratedValue` AST node to its integer ordinal.
///
/// For qualified values (`COLOR#GREEN`), uses the explicit type name.
/// For unqualified values (`GREEN`), uses the reverse lookup.
pub(crate) fn resolve_enum_ordinal(
    map: &EnumOrdinalMap,
    ev: &EnumeratedValue,
) -> Result<i32, Diagnostic> {
    let value_upper = ev.value.to_string().to_uppercase();

    if let Some(type_name) = &ev.type_name {
        // Qualified: COLOR#GREEN
        let type_upper = type_name.to_string().to_uppercase();
        map.ordinals
            .get(&(type_upper, value_upper))
            .copied()
            .ok_or_else(|| {
                Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(ev.span(), "Unknown qualified enum value"),
                )
            })
    } else {
        // Unqualified: GREEN
        map.value_lookup
            .get(&value_upper)
            .map(|(_, ordinal)| *ordinal)
            .ok_or_else(|| {
                Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(ev.span(), "Unknown enum value"),
                )
            })
    }
}

/// Returns the default ordinal for a named enumeration type.
///
/// Uses the type declaration's default value if present, otherwise 0
/// (the first declared value).
pub(crate) fn resolve_enum_default_ordinal(map: &EnumOrdinalMap, type_name: &str) -> i32 {
    let type_upper = type_name.to_uppercase();
    map.defaults.get(&type_upper).copied().unwrap_or(0)
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
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::CompilerOptions;

    fn parse_library(source: &str) -> Library {
        ironplc_parser::parse_program(source, &FileId::default(), &CompilerOptions::default())
            .unwrap()
    }

    #[test]
    fn build_enum_ordinal_map_when_simple_enum_then_assigns_ordinals() {
        let lib = parse_library(
            "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(map.ordinals.get(&("COLOR".into(), "RED".into())), Some(&0));
        assert_eq!(
            map.ordinals.get(&("COLOR".into(), "GREEN".into())),
            Some(&1)
        );
        assert_eq!(map.ordinals.get(&("COLOR".into(), "BLUE".into())), Some(&2));
    }

    #[test]
    fn build_enum_ordinal_map_when_default_specified_then_stores_default() {
        let lib = parse_library(
            "TYPE LEVEL : (LOW, MEDIUM, HIGH) := MEDIUM; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(map.defaults.get("LEVEL"), Some(&1));
    }

    #[test]
    fn build_enum_ordinal_map_when_no_default_then_defaults_to_zero() {
        let lib = parse_library(
            "TYPE STATUS : (STOPPED, RUNNING); END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(map.defaults.get("STATUS"), Some(&0));
    }

    #[test]
    fn build_enum_ordinal_map_when_multiple_enums_then_maps_all() {
        let lib = parse_library(
            "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
             TYPE LEVEL : (LOW, HIGH) := LOW; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(map.ordinals.len(), 5);
        assert_eq!(map.ordinals.get(&("COLOR".into(), "BLUE".into())), Some(&2));
        assert_eq!(map.ordinals.get(&("LEVEL".into(), "HIGH".into())), Some(&1));
    }

    #[test]
    fn resolve_enum_ordinal_when_unqualified_then_finds_value() {
        let lib = parse_library(
            "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        let ev = EnumeratedValue::new("GREEN");
        let result = resolve_enum_ordinal(&map, &ev).unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn resolve_enum_default_ordinal_when_default_set_then_returns_it() {
        let lib = parse_library(
            "TYPE LEVEL : (LOW, MEDIUM, HIGH) := HIGH; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(resolve_enum_default_ordinal(&map, "LEVEL"), 2);
    }

    #[test]
    fn resolve_enum_default_ordinal_when_unknown_type_then_returns_zero() {
        let lib = parse_library("PROGRAM main END_PROGRAM");
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(resolve_enum_default_ordinal(&map, "NONEXISTENT"), 0);
    }

    #[test]
    fn enum_var_type_info_when_called_then_returns_dint() {
        let info = enum_var_type_info();
        assert!(matches!(info.op_width, OpWidth::W32));
        assert!(matches!(info.signedness, Signedness::Signed));
        assert_eq!(info.storage_bits, 32);
    }

    #[test]
    fn build_enum_ordinal_map_when_enum_then_stores_definitions() {
        let lib = parse_library(
            "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(
            map.definitions.get("COLOR"),
            Some(&vec![
                "RED".to_string(),
                "GREEN".to_string(),
                "BLUE".to_string()
            ])
        );
    }

    #[test]
    fn build_enum_ordinal_map_when_enum_then_populates_value_lookup() {
        let lib = parse_library(
            "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
             PROGRAM main END_PROGRAM",
        );
        let map = build_enum_ordinal_map(&lib);

        assert_eq!(map.value_lookup.get("RED"), Some(&("COLOR".to_string(), 0)));
        assert_eq!(
            map.value_lookup.get("GREEN"),
            Some(&("COLOR".to_string(), 1))
        );
        assert_eq!(
            map.value_lookup.get("BLUE"),
            Some(&("COLOR".to_string(), 2))
        );
    }
}
