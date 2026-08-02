//! Enumeration explicit values and base-type suffixes.

use super::common::*;

#[test]
fn parse_when_enum_all_members_explicit_value_then_parses() {
    // Matches real usage found in a private test corpus:
    // TYPE E_ModeLanguage :(Deutsch:=1,English:=2);
    let source = "
TYPE E_ModeLanguage : (Deutsch := 1, English := 2);
END_TYPE";
    let library = parse_text(source);

    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Enumeration);
    let values = cast!(&decl.spec_init.spec, SpecificationKind::Inline);
    assert_eq!(values.values.len(), 2);
    assert_eq!(
        values.values[0].explicit_value.as_ref().map(|v| v.to_i64()),
        Some(1)
    );
    assert_eq!(
        values.values[1].explicit_value.as_ref().map(|v| v.to_i64()),
        Some(2)
    );
    assert!(decl.spec_init.underlying_type.is_none());
}

#[test]
fn parse_when_enum_first_member_explicit_value_then_parses() {
    // Matches real cross-repo usage (TcUnit's E_AssertionType, etc.):
    // only the first member has an explicit value, the rest are
    // implicit (expected to continue from the previous value + 1).
    let source = "
TYPE E_AssertionType :
(
    Type_UNDEFINED := 0,
    Type_ANY,
    Type_BOOL
);
END_TYPE";
    let library = parse_text(source);

    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Enumeration);
    let values = cast!(&decl.spec_init.spec, SpecificationKind::Inline);
    assert_eq!(values.values.len(), 3);
    assert_eq!(
        values.values[0].explicit_value.as_ref().map(|v| v.to_i64()),
        Some(0)
    );
    assert!(values.values[1].explicit_value.is_none());
    assert!(values.values[2].explicit_value.is_none());
}

#[test]
fn parse_when_enum_base_type_suffix_then_parses() {
    // Matches real usage (TcUnit's E_AssertionType/E_XmlError): a
    // CODESYS/TwinCAT base-type suffix after the value list.
    let source = "
TYPE E_AssertionType :
(
    Type_UNDEFINED := 0,
    Type_ANY,
    Type_BOOL
) BYTE;
END_TYPE";
    let library = parse_text(source);

    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Enumeration);
    assert_eq!(
        decl.spec_init.underlying_type,
        Some(dsl::common::ElementaryTypeName::BYTE)
    );
}

#[test]
fn parse_when_enum_no_explicit_values_or_base_type_then_parses_unchanged() {
    // Regression: an ordinary enum declaration must be unaffected.
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED;
END_TYPE";
    let library = parse_text(source);

    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Enumeration);
    let values = cast!(&decl.spec_init.spec, SpecificationKind::Inline);
    assert_eq!(values.values.len(), 3);
    assert!(values.values.iter().all(|v| v.explicit_value.is_none()));
    assert!(decl.spec_init.underlying_type.is_none());
    assert_eq!(
        decl.spec_init.default.as_ref().map(|d| d.value.to_string()),
        Some("RED".to_string())
    );
}
