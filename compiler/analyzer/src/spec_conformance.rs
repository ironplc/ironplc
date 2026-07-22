//! Spec conformance tests for TwinCAT `REFERENCE TO` support (analyzer-owned
//! requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_RTO_analyzer_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test asserts every analyzer-owned
//! requirement has a test here.
//!
//! See `specs/design/reference-to-twincat.md`.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;

use crate::stages::analyze;

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

fn reference_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    }
}

/// Analyze a program and return the set of problem codes it produced.
fn analyze_codes(program: &str, options: &CompilerOptions) -> Vec<String> {
    let library = parse_program(program, &FileId::default(), options).expect("program parses");
    let (_library, context) = analyze(&[&library], options).expect("analysis returns a context");
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

/// REQ-RTO-analyzer-300: `REFERENCE TO T` resolves to a reference type — a
/// `REFERENCE TO` variable can be bound and dereferenced without any
/// "deref requires a reference type" (P2031) diagnostic, proving it resolved to
/// `IntermediateType::Reference` (the same path `REF_TO` uses).
#[spec_test(REQ_RTO_analyzer_300)]
fn analyzer_spec_req_rto_300_reference_to_resolves_to_reference_type() {
    let source = "PROGRAM Main
VAR
    x : INT;
    r : REFERENCE TO INT;
    y : INT;
END_VAR
    r REF= x;
    y := r^;
END_PROGRAM";
    let codes = analyze_codes(source, &reference_to_options());
    assert!(codes.is_empty(), "expected clean analysis, got {codes:?}");
}

/// REQ-RTO-analyzer-301: Binding a `REFERENCE TO` variable to a mismatched
/// target type is rejected with P2032, reusing the `REF_TO` compatibility rule.
#[spec_test(REQ_RTO_analyzer_301)]
fn analyzer_spec_req_rto_301_reference_bind_type_mismatch_is_rejected() {
    let source = "PROGRAM Main
VAR
    x : REAL;
    r : REFERENCE TO INT;
END_VAR
    r REF= x;
END_PROGRAM";
    let codes = analyze_codes(source, &reference_to_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::ReferenceTypeMismatch.code()),
        "expected P2032 (ReferenceTypeMismatch), got {codes:?}"
    );
}
