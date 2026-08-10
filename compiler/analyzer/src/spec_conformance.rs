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

use ironplc_dsl::common::{
    ConstantKind, FunctionBlockBodyKind, InitialValueAssignmentKind, Library, LibraryElementKind,
};
use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::{Assignment, StmtKind};
use ironplc_parser::options::{CompilerOptions, Dialect};
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
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (_library, context) = analyze(&[&library], options).unwrap();
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
    let library = parse_program(source, &FileId::default(), &reference_to_options()).unwrap();
    let folded =
        crate::xform_insert_implicit_deref::apply(library, &reference_to_options()).unwrap();
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

// ---------------------------------------------------------------------------
// Compatibility libraries (analyzer-owned requirements): an activated library
// is an additional `Library` merged ahead of user source, so its declarations
// resolve exactly like user source under their exact vendor names.
//
// See `specs/design/compatibility-libraries.md`.
// ---------------------------------------------------------------------------

/// Options that let a top-level `VAR_GLOBAL` constant provide `PI` and let a
/// `VAR` initializer be a constant expression (e.g. `PI/180.0`).
fn library_options() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    }
}

/// The `Tc2_System`-style compatibility library providing the global `PI`.
fn pi_library(options: &CompilerOptions) -> Library {
    parse_program(
        "VAR_GLOBAL CONSTANT PI : LREAL := 3.14159265358979; END_VAR",
        &FileId::default(),
        options,
    )
    .unwrap()
}

/// The folded `LREAL` initializer value of `var_name` in `fb_name`, if it
/// reduced to a real literal.
fn folded_lreal(library: &Library, fb_name: &str, var_name: &str) -> Option<f64> {
    for element in &library.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };
        if fb.name.to_string() != fb_name {
            continue;
        }
        for var in &fb.variables {
            let is_match = var
                .identifier
                .symbolic_id()
                .is_some_and(|id| id.original() == var_name);
            if !is_match {
                continue;
            }
            if let InitialValueAssignmentKind::Simple(simple) = &var.initializer {
                if let Some(ConstantKind::RealLiteral(real)) = &simple.initial_value {
                    return Some(real.value);
                }
            }
        }
    }
    None
}

/// REQ-CL-analyzer-001: A compatibility library is dormant by default — its
/// declarations are in scope only when the library is activated.
#[spec_test(REQ_CL_analyzer_001)]
fn analyzer_spec_req_cl_001_library_dormant_until_activated() {
    let options = library_options();
    let user = parse_program(
        "FUNCTION_BLOCK FB_Example VAR d2r : LREAL := PI/180.0; END_VAR END_FUNCTION_BLOCK",
        &FileId::default(),
        &options,
    )
    .unwrap();

    // Not activated: PI is not in scope, so the initializer cannot resolve.
    let (_lib, ctx) = analyze(&[&user], &options).unwrap();
    assert!(
        ctx.has_diagnostics(),
        "PI must be undefined when the library is dormant"
    );

    // Activated: injecting the library makes PI resolve cleanly.
    let library = pi_library(&options);
    let (_lib, ctx) = analyze(&[&library, &user], &options).unwrap();
    assert!(
        !ctx.has_diagnostics(),
        "unexpected diagnostics once activated: {:?}",
        ctx.diagnostics()
    );
}

/// REQ-CL-analyzer-002: An activated library's symbols resolve under their exact
/// vendor names (flat), with no compiler-injected namespace qualifier.
#[spec_test(REQ_CL_analyzer_002)]
fn analyzer_spec_req_cl_002_symbols_resolve_flat() {
    let options = library_options();
    let library = pi_library(&options);
    // The source writes the bare, unqualified name `PI` (not `Tc2_System.PI`).
    let user = parse_program(
        "FUNCTION_BLOCK FB_Example VAR half : LREAL := PI/2.0; END_VAR END_FUNCTION_BLOCK",
        &FileId::default(),
        &options,
    )
    .unwrap();
    let (analyzed, ctx) = analyze(&[&library, &user], &options).unwrap();
    assert!(
        !ctx.has_diagnostics(),
        "bare `PI` must resolve flat: {:?}",
        ctx.diagnostics()
    );
    let value = folded_lreal(&analyzed, "FB_Example", "half").unwrap();
    assert!(
        (value - std::f64::consts::PI / 2.0).abs() < 1e-9,
        "flat `PI` did not resolve to the library value, got {value}"
    );
}

/// REQ-CL-analyzer-003: When a math library is active, `PI` resolves as a
/// constant and folds at compile time, so it is usable in a `VAR` initializer.
#[spec_test(REQ_CL_analyzer_003)]
fn analyzer_spec_req_cl_003_pi_folds_in_initializer() {
    let options = library_options();
    let library = pi_library(&options);
    let user = parse_program(
        "FUNCTION_BLOCK FB_Example VAR d2r : LREAL := PI/180.0; END_VAR END_FUNCTION_BLOCK",
        &FileId::default(),
        &options,
    )
    .unwrap();
    let (analyzed, ctx) = analyze(&[&library, &user], &options).unwrap();
    assert!(
        !ctx.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        ctx.diagnostics()
    );
    let value = folded_lreal(&analyzed, "FB_Example", "d2r").unwrap();
    assert!(
        (value - std::f64::consts::PI / 180.0).abs() < 1e-9,
        "PI/180.0 did not fold to the expected value, got {value}"
    );
}

/// REQ-CL-analyzer-004: A user declaration shadows an activated library
/// declaration of the same name.
#[spec_test(REQ_CL_analyzer_004)]
fn analyzer_spec_req_cl_004_user_declaration_shadows_library() {
    let options = library_options();
    let library = pi_library(&options); // library global PI = 3.14159...
                                        // The FB declares its own local CONSTANT PI, which must win.
    let user = parse_program(
        "FUNCTION_BLOCK FB_Example \
         VAR CONSTANT PI : LREAL := 2.0; END_VAR \
         VAR d2r : LREAL := PI/180.0; END_VAR \
         END_FUNCTION_BLOCK",
        &FileId::default(),
        &options,
    )
    .unwrap();
    let (analyzed, ctx) = analyze(&[&library, &user], &options).unwrap();
    assert!(
        !ctx.has_diagnostics(),
        "a user declaration shadowing a library one must not error: {:?}",
        ctx.diagnostics()
    );
    let value = folded_lreal(&analyzed, "FB_Example", "d2r").unwrap();
    // The local PI (2.0) shadowed the library global (3.14159...).
    assert!(
        (value - 2.0 / 180.0).abs() < 1e-12,
        "expected the local PI (2.0) to shadow the library global, got {value}"
    );
}

/// REQ-CL-analyzer-006: Selecting a dialect does not activate any compatibility
/// library; library activation comes only from the activation channels.
#[spec_test(REQ_CL_analyzer_006)]
fn analyzer_spec_req_cl_006_dialect_does_not_activate_library() {
    // The TwinCAT dialect enables top-level VAR_GLOBAL and more, but it must NOT
    // make `PI` resolve on its own — no library has been activated.
    let mut options = CompilerOptions::from_dialect(Dialect::TwinCat);
    options.allow_constant_initializer_expressions = true;
    let user = parse_program(
        "FUNCTION_BLOCK FB_Example VAR d2r : LREAL := PI/180.0; END_VAR END_FUNCTION_BLOCK",
        &FileId::default(),
        &options,
    )
    .unwrap();
    let (_lib, ctx) = analyze(&[&user], &options).unwrap();
    assert!(
        ctx.has_diagnostics(),
        "selecting a dialect must not activate the library"
    );
}

/// REQ-CL-analyzer-007: A program that *calls* a declare-only library POU
/// still passes `check` cleanly — the analyzer never sees bindings, so the
/// declaration resolves and type-checks like any other function. (Rejecting
/// the call is codegen's job: P4046, REQ-CL-codegen-002.)
#[spec_test(REQ_CL_analyzer_007)]
fn analyzer_spec_req_cl_007_declare_only_call_passes_check() {
    let options = CompilerOptions::default();
    // The library declaration as it appears in a version's `.st`: the full
    // interface with a body of exactly `;`. The binding itself rides a
    // side-table to codegen and is invisible here.
    let library = parse_program(
        "FUNCTION LREAL_TO_FMTSTR : STRING[255]
VAR_INPUT
    in         : LREAL;
    iPrecision : INT;
    bRound     : BOOL;
END_VAR
;
END_FUNCTION",
        &FileId::from_string("lib.st"),
        &options,
    )
    .unwrap();
    let user = parse_program(
        "PROGRAM main
VAR
    s : STRING[255];
END_VAR
    s := LREAL_TO_FMTSTR(in := 1.5, iPrecision := 2, bRound := TRUE);
END_PROGRAM",
        &FileId::from_string("user.st"),
        &options,
    )
    .unwrap();
    let (_lib, ctx) = analyze(&[&library, &user], &options).unwrap();
    assert!(
        !ctx.has_diagnostics(),
        "a declare-only call must pass check: {:?}",
        ctx.diagnostics()
    );
}
