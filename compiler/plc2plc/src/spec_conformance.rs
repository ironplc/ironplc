//! Spec conformance tests for TwinCAT `REFERENCE TO` support (plc2plc-owned
//! requirements): round-trip rendering of the surface syntax.
//!
//! Each test is annotated with `#[spec_test(REQ_RTO_plc2plc_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test asserts every plc2plc-owned
//! requirement has a test here.
//!
//! See `specs/design/reference-to-twincat.md`.

use dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use spec_test_macro::spec_test;

use crate::write_to_string;

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

fn render(source: &str, options: &CompilerOptions) -> String {
    let library = parse_program(source, &FileId::default(), options).expect("program parses");
    write_to_string(&library).expect("library renders")
}

fn reference_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    }
}

/// REQ-RTO-plc2plc-600: A `ReferenceTo`-tagged declaration renders as
/// `REFERENCE TO <target>`.
#[spec_test(REQ_RTO_plc2plc_600)]
fn plc2plc_spec_req_rto_600_reference_to_declaration_renders() {
    let rendered = render(
        "TYPE T : REFERENCE TO INT; END_TYPE",
        &reference_to_options(),
    );
    assert!(
        rendered.contains("REFERENCE TO INT"),
        "expected `REFERENCE TO INT` in:\n{rendered}"
    );
    assert!(
        !rendered.contains("REF_TO"),
        "REFERENCE TO must not render as REF_TO:\n{rendered}"
    );
}

/// REQ-RTO-plc2plc-601: A `REF=` binding renders back as `REF=`.
#[spec_test(REQ_RTO_plc2plc_601)]
fn plc2plc_spec_req_rto_601_ref_assign_renders() {
    let source = "PROGRAM main
VAR
    x : INT;
    r : REFERENCE TO INT;
END_VAR
    r REF= x;
END_PROGRAM";
    let rendered = render(source, &reference_to_options());
    assert!(
        rendered.contains("REF="),
        "expected `REF=` binding in:\n{rendered}"
    );
}

/// REQ-RTO-plc2plc-602: A `RefTo`-tagged declaration still renders as `REF_TO`
/// (regression).
#[spec_test(REQ_RTO_plc2plc_602)]
fn plc2plc_spec_req_rto_602_ref_to_still_renders() {
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let rendered = render("TYPE T : REF_TO INT; END_TYPE", &options);
    assert!(
        rendered.contains("REF_TO INT"),
        "expected `REF_TO INT` in:\n{rendered}"
    );
    assert!(
        !rendered.contains("REFERENCE"),
        "REF_TO must not render as REFERENCE TO:\n{rendered}"
    );
}
