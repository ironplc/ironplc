//! Spec conformance tests for TwinCAT/CODESYS `POINTER TO` support
//! (analyzer-owned requirements): explicit-dereference semantics.
//!
//! Each test is annotated with `#[spec_test(REQ_PTR_analyzer_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every analyzer-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use ironplc_dsl::common::{FunctionBlockBodyKind, Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::{ExprKind, StmtKind};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;

use crate::stages::analyze;

/// `allow_pointer_to` plus `allow_ref_to`, the combination the `codesys`
/// dialect provides: `REF()`/`NULL` are the only binding forms until `ADR`
/// lands (Phase 2).
fn pointer_options() -> CompilerOptions {
    CompilerOptions {
        allow_pointer_to: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    }
}

/// Analyze a program and return the set of problem codes it produced.
fn analyze_codes(program: &str, options: &CompilerOptions) -> Vec<String> {
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (_library, context) = analyze(&[&library], options).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
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

/// REQ-PTR-analyzer-300: Reading through an explicit dereference of a
/// `POINTER TO` variable is accepted — the pointer resolves to the same
/// reference intermediate type as `REF_TO`, with no P2031 diagnostic.
#[spec_test(REQ_PTR_analyzer_300)]
fn analyzer_spec_req_ptr_300_explicit_deref_of_pointer_is_accepted() {
    let source = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT;
    y : INT;
END_VAR
    p := REF(x);
    y := p^;
    p^ := 42;
END_PROGRAM";
    let codes = analyze_codes(source, &pointer_options());
    assert!(codes.is_empty(), "expected clean analysis, got {codes:?}");
}

/// REQ-PTR-analyzer-301: A bare use of a `POINTER TO` variable is not
/// implicitly dereferenced, even when `allow_reference_to` is also enabled —
/// the implicit-deref transform keys on `RefSyntax::ReferenceTo` and leaves
/// `PointerTo` variables on explicit `^`.
#[spec_test(REQ_PTR_analyzer_301)]
fn analyzer_spec_req_ptr_301_pointer_is_not_implicitly_dereferenced() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM Main
VAR
    p : POINTER TO INT;
    r : REFERENCE TO INT;
    y : INT;
END_VAR
    y := p;
    y := r;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, _context) = analyze(&[&library], &options).unwrap();
    let statements = program_statements(&analyzed);
    let StmtKind::Assignment(pointer_read) = &statements[0] else {
        panic!("expected assignment");
    };
    assert!(
        matches!(pointer_read.value.kind, ExprKind::Variable(_)),
        "a bare read of a POINTER TO variable must stay un-dereferenced, got {:?}",
        pointer_read.value.kind
    );
    // Control: the REFERENCE TO variable in the same program *is* wrapped.
    let StmtKind::Assignment(reference_read) = &statements[1] else {
        panic!("expected assignment");
    };
    assert!(
        matches!(reference_read.value.kind, ExprKind::Deref(_)),
        "a bare read of a REFERENCE TO variable must be auto-dereferenced"
    );
}

/// REQ-PTR-analyzer-302: Binding a `POINTER TO T` variable to a reference of
/// a different base type is rejected with P2032, reusing the `REF_TO`
/// compatibility rule.
#[spec_test(REQ_PTR_analyzer_302)]
fn analyzer_spec_req_ptr_302_pointer_bind_type_mismatch_is_rejected() {
    let source = "PROGRAM Main
VAR
    x : REAL;
    p : POINTER TO INT;
END_VAR
    p := REF(x);
END_PROGRAM";
    let codes = analyze_codes(source, &pointer_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::ReferenceTypeMismatch.code()),
        "expected P2032 (ReferenceTypeMismatch), got {codes:?}"
    );
}

/// REQ-PTR-analyzer-303: Arithmetic on a `POINTER TO` value is rejected with
/// P2033 unless `allow_ref_arithmetic` is set — pointer arithmetic cannot be
/// mapped onto variable-table indices.
#[spec_test(REQ_PTR_analyzer_303)]
fn analyzer_spec_req_ptr_303_pointer_arithmetic_is_rejected() {
    let source = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT := REF(x);
    y : INT;
END_VAR
    y := p + 1;
END_PROGRAM";
    let codes = analyze_codes(source, &pointer_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::ArithmeticOnReference.code()),
        "expected P2033 (ArithmeticOnReference), got {codes:?}"
    );
}

/// REQ-PTR-analyzer-304: `NULL` may be assigned to a `POINTER TO` variable
/// (the `NULL` keyword comes from `allow_ref_to`, as in the `codesys`
/// dialect).
#[spec_test(REQ_PTR_analyzer_304)]
fn analyzer_spec_req_ptr_304_null_assignment_to_pointer_is_accepted() {
    let source = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT := REF(x);
END_VAR
    p := NULL;
END_PROGRAM";
    let codes = analyze_codes(source, &pointer_options());
    assert!(codes.is_empty(), "expected clean analysis, got {codes:?}");
}
