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

use ironplc_dsl::common::{FunctionBlockBodyKind, Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::{Assignment, StmtKind};
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

/// Returns the statements of the (single) PROGRAM in a library.
fn program_statements(lib: &Library) -> Vec<StmtKind> {
    for element in &lib.elements {
        if let LibraryElementKind::ProgramDeclaration(prog) = element {
            let FunctionBlockBodyKind::Statements(stmts) = &prog.body else {
                panic!("program body is not a statement list");
            };
            return stmts.body.clone();
        }
    }
    panic!("no program declaration found");
}

/// REQ-RTO-analyzer-502: The target of a `REF=` binding is not auto-dereferenced
/// — the implicit-dereference transform leaves the binding assignment untouched
/// (`deref` stays false), while a bare `:=` write to the same variable does get
/// the dereferencing store (`deref` becomes true).
#[spec_test(REQ_RTO_analyzer_502)]
fn analyzer_spec_req_rto_502_ref_assign_target_is_not_dereferenced() {
    let source = "PROGRAM Main
VAR
    x : INT;
    r : REFERENCE TO INT;
END_VAR
    r REF= x;
    r := 5;
END_PROGRAM";
    let library =
        parse_program(source, &FileId::default(), &reference_to_options()).expect("program parses");
    let folded = crate::xform_insert_implicit_deref::apply(library, &reference_to_options())
        .expect("transform succeeds");
    let statements = program_statements(&folded);
    let assignments: Vec<&Assignment> = statements
        .iter()
        .filter_map(|s| match s {
            StmtKind::Assignment(a) => Some(a),
            _ => None,
        })
        .collect();
    assert_eq!(assignments.len(), 2, "expected two assignments");
    // `r REF= x` rebinds the reference itself and must not be auto-dereferenced.
    assert!(
        assignments[0].ref_bind,
        "first assignment is a REF= binding"
    );
    assert!(
        !assignments[0].deref,
        "REF= binding target must not be auto-dereferenced"
    );
    // `r := 5` is a bare write and must store through the reference.
    assert!(!assignments[1].ref_bind);
    assert!(
        assignments[1].deref,
        "bare write to a REFERENCE TO variable must be auto-dereferenced"
    );
}

/// REQ-RTO-analyzer-505: `__ISVALIDREF` is recognized (and lowered) only when
/// `allow_reference_to` is set. With the flag off it stays an ordinary
/// identifier and the call is reported as an undeclared function.
#[spec_test(REQ_RTO_analyzer_505)]
fn analyzer_spec_req_rto_505_isvalidref_recognized_only_with_flag() {
    // Flag off: __ISVALIDREF is not a builtin, so the call is undeclared.
    let source_off = "PROGRAM Main
VAR
    x : INT;
    b : BOOL;
END_VAR
    b := __ISVALIDREF(x);
END_PROGRAM";
    let codes = analyze_codes(source_off, &CompilerOptions::default());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::FunctionCallUndeclared.code()),
        "without the flag __ISVALIDREF must be an undeclared function, got {codes:?}"
    );

    // Flag on: __ISVALIDREF is lowered to `r <> NULL`, so it is not undeclared.
    let source_on = "PROGRAM Main
VAR
    x : INT;
    r : REFERENCE TO INT;
    b : BOOL;
END_VAR
    r REF= x;
    b := __ISVALIDREF(r);
END_PROGRAM";
    let codes = analyze_codes(source_on, &reference_to_options());
    assert!(
        !codes
            .iter()
            .any(|c| c.as_str() == Problem::FunctionCallUndeclared.code()),
        "with the flag __ISVALIDREF must be recognized (lowered), got {codes:?}"
    );
}
