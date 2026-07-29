//! Spec conformance tests for TwinCAT `REFERENCE TO` support (parser-owned
//! requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_RTO_parser_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test asserts every parser-owned
//! requirement has a test here.
//!
//! See `specs/design/reference-to-twincat.md`.

use dsl::common::{
    DataTypeDeclarationKind, FunctionBlockBodyKind, InitialValueAssignmentKind, Library,
    LibraryElementKind, RefSyntax, SpecificationKind,
};
use dsl::core::FileId;
use dsl::textual::{ExprKind, StmtKind};
use ironplc_test::cast;
use spec_test_macro::spec_test;

use crate::options::{CompilerOptions, Dialect};
use crate::token::TokenType;

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn reference_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    }
}

fn parse(source: &str, options: &CompilerOptions) -> Library {
    crate::parse_program(source, &FileId::default(), options).expect("program should parse")
}

fn token_types(source: &str, options: &CompilerOptions) -> Vec<TokenType> {
    let (tokens, _errors) = crate::tokenize_program(source, &FileId::default(), options, 0, 0);
    tokens.iter().map(|t| t.token_type.clone()).collect()
}

// ---------------------------------------------------------------------------
// Options & dialects
// ---------------------------------------------------------------------------

/// REQ-RTO-parser-001: The `codesys` dialect enables `allow_reference_to`.
#[spec_test(REQ_RTO_parser_001)]
fn options_spec_req_rto_001_codesys_enables_reference_to() {
    assert!(CompilerOptions::from_dialect(Dialect::Codesys).allow_reference_to);
}

/// REQ-RTO-parser-002: The `rusty` dialect does not enable `allow_reference_to`.
#[spec_test(REQ_RTO_parser_002)]
fn options_spec_req_rto_002_rusty_does_not_enable_reference_to() {
    assert!(!CompilerOptions::from_dialect(Dialect::Rusty).allow_reference_to);
}

/// REQ-RTO-parser-003: Setting both `allow_reference_to` and `allow_ref_to`
/// together is accepted — no combination is rejected (ADR-0038).
#[spec_test(REQ_RTO_parser_003)]
fn options_spec_req_rto_003_reference_to_and_ref_to_coexist() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    // A program using both syntaxes parses without any combination error.
    let source = "TYPE A : REF_TO INT; END_TYPE
TYPE B : REFERENCE TO INT; END_TYPE";
    let lib = parse(source, &options);
    assert_eq!(lib.elements.len(), 2);
}

// ---------------------------------------------------------------------------
// Lexer & keyword demotion
// ---------------------------------------------------------------------------

/// REQ-RTO-parser-100: `REFERENCE` lexes as a single `Reference` keyword token
/// (distinct from `REF`).
#[spec_test(REQ_RTO_parser_100)]
fn lexer_spec_req_rto_100_reference_lexes_as_reference_token() {
    let types = token_types("REFERENCE", &reference_to_options());
    assert!(types.contains(&TokenType::Reference));
    assert!(!types.contains(&TokenType::Ref));
}

/// REQ-RTO-parser-101: With the flag off, `REFERENCE` is demoted to
/// `Identifier`.
#[spec_test(REQ_RTO_parser_101)]
fn xform_spec_req_rto_101_reference_demoted_when_flag_off() {
    let types = token_types("REFERENCE", &CompilerOptions::default());
    assert!(types.contains(&TokenType::Identifier));
    assert!(!types.contains(&TokenType::Reference));
}

/// REQ-RTO-parser-102: With the flag on, `REFERENCE` stays the `Reference`
/// keyword.
#[spec_test(REQ_RTO_parser_102)]
fn xform_spec_req_rto_102_reference_kept_when_flag_on() {
    let types = token_types("REFERENCE", &reference_to_options());
    assert!(types.contains(&TokenType::Reference));
}

/// REQ-RTO-parser-103: `REFERENCE` is a valid identifier in standard mode.
#[spec_test(REQ_RTO_parser_103)]
fn parser_spec_req_rto_103_reference_is_identifier_in_standard_mode() {
    let source = "PROGRAM main
VAR
    REFERENCE : INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &CompilerOptions::default());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let name = prog.variables[0]
        .identifier
        .symbolic_id()
        .expect("variable has a symbolic name");
    assert_eq!(name.to_string(), "REFERENCE");
}

// ---------------------------------------------------------------------------
// Parser productions
// ---------------------------------------------------------------------------

/// REQ-RTO-parser-200: `r : REFERENCE TO INT;` yields an initializer tagged
/// `RefSyntax::ReferenceTo`.
#[spec_test(REQ_RTO_parser_200)]
fn parser_spec_req_rto_200_reference_to_var_decl_is_tagged() {
    let source = "PROGRAM main
VAR
    r : REFERENCE TO INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &reference_to_options());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert_eq!(init.syntax, RefSyntax::ReferenceTo);
}

/// REQ-RTO-parser-201: `TYPE T : REFERENCE TO INT; END_TYPE` yields a
/// declaration tagged `RefSyntax::ReferenceTo`.
#[spec_test(REQ_RTO_parser_201)]
fn parser_spec_req_rto_201_reference_to_type_decl_is_tagged() {
    let lib = parse(
        "TYPE T : REFERENCE TO INT; END_TYPE",
        &reference_to_options(),
    );
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.syntax, RefSyntax::ReferenceTo);
}

/// REQ-RTO-parser-202: A `REF_TO` declaration is tagged `RefSyntax::RefTo`.
#[spec_test(REQ_RTO_parser_202)]
fn parser_spec_req_rto_202_ref_to_is_tagged_ref_to() {
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let lib = parse("TYPE T : REF_TO INT; END_TYPE", &options);
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.syntax, RefSyntax::RefTo);
}

/// REQ-RTO-parser-210: `r REF= x;` parses as a reference binding equivalent to
/// `r := REF(x)` (value is `ExprKind::Ref`).
#[spec_test(REQ_RTO_parser_210)]
fn parser_spec_req_rto_210_ref_assign_parses_as_reference_binding() {
    let source = "PROGRAM main
VAR
    r : REFERENCE TO INT;
    x : INT;
END_VAR
    r REF= x;
END_PROGRAM";
    let lib = parse(source, &reference_to_options());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let stmts = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    let assignment = cast!(&stmts.body[0], StmtKind::Assignment);
    assert!(assignment.ref_bind);
    let referent = cast!(&assignment.value.kind, ExprKind::Ref);
    assert_eq!(referent.to_string(), "x");
}

/// REQ-RTO-parser-220: `ARRAY [..] OF REFERENCE TO T` tags the element
/// `Some(RefSyntax::ReferenceTo)`.
#[spec_test(REQ_RTO_parser_220)]
fn parser_spec_req_rto_220_array_of_reference_to_is_tagged() {
    let source = "PROGRAM main
VAR
    a : ARRAY[0..3] OF REFERENCE TO INT;
END_VAR
END_PROGRAM";
    let lib = parse(source, &reference_to_options());
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.ref_to, Some(RefSyntax::ReferenceTo));
}
