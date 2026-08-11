//! Spec conformance tests for TwinCAT/CODESYS `POINTER TO` support
//! (parser-owned requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_PTR_parser_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every parser-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use dsl::common::{
    DataTypeDeclarationKind, InitialValueAssignmentKind, Library, LibraryElementKind, RefSyntax,
    SpecificationKind,
};
use dsl::core::FileId;
use ironplc_test::cast;
use spec_test_macro::spec_test;

use crate::options::{CompilerOptions, Dialect};
use crate::token::TokenType;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pointer_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    }
}

fn parse(source: &str, options: &CompilerOptions) -> Library {
    crate::parse_program(source, &FileId::default(), options).unwrap()
}

fn token_types(source: &str, options: &CompilerOptions) -> Vec<TokenType> {
    let (tokens, _errors) = crate::tokenize_program(source, &FileId::default(), options, 0, 0);
    tokens.iter().map(|t| t.token_type.clone()).collect()
}

// ---------------------------------------------------------------------------
// Options & dialects
// ---------------------------------------------------------------------------

/// REQ-PTR-parser-001: The `codesys` dialect enables `allow_pointer_to`.
#[spec_test(REQ_PTR_parser_001)]
fn options_spec_req_ptr_001_codesys_enables_pointer_to() {
    assert!(CompilerOptions::from_dialect(Dialect::Codesys).allow_pointer_to);
}

/// REQ-PTR-parser-002: The `twincat` dialect enables `allow_pointer_to`.
#[spec_test(REQ_PTR_parser_002)]
fn options_spec_req_ptr_002_twincat_enables_pointer_to() {
    assert!(CompilerOptions::from_dialect(Dialect::TwinCat).allow_pointer_to);
}

/// REQ-PTR-parser-003: The `iec61131-3-ed2`, `iec61131-3-ed3`, and `rusty`
/// dialects do not enable `allow_pointer_to`.
#[spec_test(REQ_PTR_parser_003)]
fn options_spec_req_ptr_003_other_dialects_do_not_enable_pointer_to() {
    assert!(!CompilerOptions::from_dialect(Dialect::Iec61131_3Ed2).allow_pointer_to);
    assert!(!CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3).allow_pointer_to);
    assert!(!CompilerOptions::from_dialect(Dialect::Rusty).allow_pointer_to);
}

// ---------------------------------------------------------------------------
// Lexer & keyword demotion
// ---------------------------------------------------------------------------

/// REQ-PTR-parser-100: `POINTER` lexes as a single `Pointer` keyword token.
#[spec_test(REQ_PTR_parser_100)]
fn lexer_spec_req_ptr_100_pointer_lexes_as_pointer_token() {
    let types = token_types("POINTER", &pointer_to_options());
    assert!(types.contains(&TokenType::Pointer));
}

/// REQ-PTR-parser-101: With the flag off, `POINTER` is demoted to
/// `Identifier`.
#[spec_test(REQ_PTR_parser_101)]
fn xform_spec_req_ptr_101_pointer_demoted_when_flag_off() {
    let types = token_types("POINTER", &CompilerOptions::default());
    assert!(types.contains(&TokenType::Identifier));
    assert!(!types.contains(&TokenType::Pointer));
}

/// REQ-PTR-parser-102: With the flag on, `POINTER` stays the `Pointer`
/// keyword.
#[spec_test(REQ_PTR_parser_102)]
fn xform_spec_req_ptr_102_pointer_kept_when_flag_on() {
    let types = token_types("POINTER", &pointer_to_options());
    assert!(types.contains(&TokenType::Pointer));
    assert!(!types.contains(&TokenType::Identifier));
}

/// REQ-PTR-parser-103: `POINTER` is a valid identifier in standard mode.
#[spec_test(REQ_PTR_parser_103)]
fn parser_spec_req_ptr_103_pointer_is_identifier_in_standard_mode() {
    let source = "PROGRAM main
VAR
    POINTER : INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &CompilerOptions::default());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let name = prog.variables[0].identifier.symbolic_id().unwrap();
    assert_eq!(name.to_string(), "POINTER");
}

// ---------------------------------------------------------------------------
// Parser productions
// ---------------------------------------------------------------------------

/// REQ-PTR-parser-200: `p : POINTER TO INT;` yields an initializer tagged
/// `RefSyntax::PointerTo`.
#[spec_test(REQ_PTR_parser_200)]
fn parser_spec_req_ptr_200_pointer_to_var_decl_is_tagged() {
    let source = "PROGRAM main
VAR
    p : POINTER TO INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &pointer_to_options());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert_eq!(init.syntax, RefSyntax::PointerTo);
}

/// REQ-PTR-parser-201: `TYPE T : POINTER TO INT; END_TYPE` yields a
/// declaration tagged `RefSyntax::PointerTo`.
#[spec_test(REQ_PTR_parser_201)]
fn parser_spec_req_ptr_201_pointer_to_type_decl_is_tagged() {
    let lib = parse("TYPE T : POINTER TO INT; END_TYPE", &pointer_to_options());
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.syntax, RefSyntax::PointerTo);
}

/// REQ-PTR-parser-210: `ARRAY [..] OF POINTER TO T` tags the element
/// `Some(RefSyntax::PointerTo)`.
#[spec_test(REQ_PTR_parser_210)]
fn parser_spec_req_ptr_210_array_of_pointer_to_is_tagged() {
    let source = "PROGRAM main
VAR
    a : ARRAY[0..3] OF POINTER TO INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &pointer_to_options());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.ref_to, Some(RefSyntax::PointerTo));
}

/// REQ-PTR-parser-211: With the flag off, `p : POINTER TO INT;` is a syntax
/// error.
#[spec_test(REQ_PTR_parser_211)]
fn parser_spec_req_ptr_211_pointer_to_rejected_when_flag_off() {
    let source = "PROGRAM main
VAR
    p : POINTER TO INT;
END_VAR
END_PROGRAM";
    let result = crate::parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
