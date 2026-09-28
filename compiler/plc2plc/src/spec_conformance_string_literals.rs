//! Spec conformance tests for character string literals (plc2plc-owned
//! requirements): a rendered literal re-parses to the same characters.
//!
//! Each test is annotated with `#[spec_test(REQ_SL_plc2plc_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every plc2plc-owned requirement has a test.
//!
//! See `specs/design/string-literals.md`.

use dsl::common::{
    CharacterStringLiteral, DataTypeDeclarationKind, Integer, IntegerRef, Library,
    LibraryElementKind, StringDeclaration, TypeName,
};
use dsl::core::{FileId, SourceSpan};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::write_to_string;

/// A library declaring a string type whose default is `value`.
fn library_with_default(value: Vec<char>, wide: bool) -> Library {
    let init = if wide {
        CharacterStringLiteral::new_wide(value)
    } else {
        CharacterStringLiteral::new(value)
    };
    Library {
        elements: vec![LibraryElementKind::DataTypeDeclaration(
            DataTypeDeclarationKind::String(StringDeclaration {
                type_name: TypeName::from("S"),
                length: IntegerRef::Literal(Integer {
                    span: SourceSpan::default(),
                    value: 1000,
                }),
                width: init.width.clone(),
                init: Some(init),
            }),
        )],
    }
}

/// REQ-SL-plc2plc-001: Every character from U+0000 to U+01FF, in either
/// width, renders to text that re-parses to the same characters.
#[rstest]
#[case::narrow(false)]
#[case::wide(true)]
#[spec_test(REQ_SL_plc2plc_001)]
fn plc2plc_spec_req_sl_001_rendered_literal_reparses_to_same_characters(#[case] wide: bool) {
    let value: Vec<char> = (0u32..=0x1FF).filter_map(char::from_u32).collect();
    let rendered = write_to_string(&library_with_default(value.clone(), wide)).unwrap();
    let reparsed =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).unwrap();
    let reparsed_value = match &reparsed.elements[0] {
        LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::String(decl)) => {
            decl.init.as_ref().map(|lit| lit.value.clone())
        }
        _ => None,
    };
    assert_eq!(Some(value), reparsed_value, "rendered:\n{rendered}");
}
