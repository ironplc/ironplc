//! Spec conformance tests for TwinCAT/CODESYS `POINTER TO` support
//! (plc2plc-owned requirements): round-trip rendering of the surface syntax.
//!
//! Each test is annotated with `#[spec_test(REQ_PTR_plc2plc_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every plc2plc-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use spec_test_macro::spec_test;

use crate::write_to_string;

fn render(source: &str, options: &CompilerOptions) -> String {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let rendered = write_to_string(&library).unwrap();

    // Rendering is only half the requirement: what comes out has to be valid
    // input, so every conformance render re-parses.
    let reparsed = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    assert_eq!(
        library, reparsed,
        "Round trip changed the AST. Rendered:\n{rendered}"
    );

    rendered
}

/// REQ-PTR-plc2plc-600: A `PointerTo`-tagged declaration renders as
/// `POINTER TO <target>`.
#[spec_test(REQ_PTR_plc2plc_600)]
fn plc2plc_spec_req_ptr_600_pointer_to_declaration_renders() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let rendered = render("TYPE T : POINTER TO INT; END_TYPE", &options);
    assert!(
        rendered.contains("POINTER TO INT"),
        "expected `POINTER TO INT` in:\n{rendered}"
    );
    assert!(
        !rendered.contains("REF_TO"),
        "POINTER TO must not render as REF_TO:\n{rendered}"
    );
}

/// REQ-PTR-plc2plc-601: `REF_TO`, `REFERENCE TO`, and `POINTER TO`
/// declarations in one program each render with their own spelling preserved.
#[spec_test(REQ_PTR_plc2plc_601)]
fn plc2plc_spec_req_ptr_601_all_three_spellings_render_distinctly() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_reference_to: true,
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM main
VAR
    a : REF_TO INT;
    b : REFERENCE TO INT;
    c : POINTER TO INT;
END_VAR
END_PROGRAM";
    let rendered = render(source, &options);
    assert!(
        rendered.contains("a : REF_TO INT"),
        "expected `REF_TO` spelling preserved in:\n{rendered}"
    );
    assert!(
        rendered.contains("b : REFERENCE TO INT"),
        "expected `REFERENCE TO` spelling preserved in:\n{rendered}"
    );
    assert!(
        rendered.contains("c : POINTER TO INT"),
        "expected `POINTER TO` spelling preserved in:\n{rendered}"
    );
}
