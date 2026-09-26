//! Type declarations that alias an elementary type (`MY_ALIAS : INT;`).

use super::common::*;
use dsl::common::ElementaryTypeName;

// An elementary base type is a keyword, so the declaration is not ambiguous
// the way `NAME : NAME` is: it is a simple declaration without an initial
// value, not a late-bound one (#1416).
#[rstest]
#[case::int("INT", ElementaryTypeName::INT)]
#[case::bool("BOOL", ElementaryTypeName::BOOL)]
#[case::real("REAL", ElementaryTypeName::REAL)]
#[case::time("TIME", ElementaryTypeName::TIME)]
#[case::string("STRING", ElementaryTypeName::STRING)]
fn parse_when_type_alias_of_elementary_without_initializer_then_simple_declaration(
    #[case] base: &str,
    #[case] expected: ElementaryTypeName,
) {
    let source = format!("TYPE MY_ALIAS : {base}; END_TYPE");
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(library.is_ok(), "{library:?}");
    let library = library.unwrap();
    let decl = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let simple = cast!(decl, DataTypeDeclarationKind::Simple);
    assert_eq!(simple.type_name, TypeName::from("MY_ALIAS"));
    let init = cast!(&simple.spec_and_init, InitialValueAssignmentKind::Simple);
    let expected: TypeName = expected.into();
    assert_eq!(init.type_name, expected);
    assert!(init.initial_value.is_none());
}
