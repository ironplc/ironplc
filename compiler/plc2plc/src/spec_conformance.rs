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

// ---------------------------------------------------------------------------
// Compatibility libraries (plc2plc-owned requirement).
//
// See `specs/design/compatibility-libraries.md`. An activated library injects
// its declarations into semantic analysis only — as a *separate* `Library`
// that is merged for type resolution but never handed to `plc2plc`. `plc2plc`
// renders exactly the library it is given, so rendering the user's source
// reproduces it unchanged and never emits the injected declarations.
// ---------------------------------------------------------------------------

/// REQ-CL-plc2plc-001: `plc2plc` emits the user's source unchanged; declarations
/// injected by an activated library are never rendered as user source.
#[spec_test(REQ_CL_plc2plc_001)]
fn plc2plc_spec_req_cl_001_injected_declarations_not_rendered() {
    // A user POU that *uses* `PI`, exactly as a TwinCAT source would. `PI` is a
    // constant the activated `Tc2_System` library provides; the user never
    // writes its declaration.
    let user_source = "PROGRAM main
VAR
    d2r : LREAL := PI / 180.0;
END_VAR
END_PROGRAM";

    // The declarations an activated library injects: `Tc2_System`'s global
    // constant `PI`. In a real compile these are parsed into a *separate*
    // `Library` (see `ironplc_sources::libraries`) that is merged for analysis
    // only; `plc2plc` is only ever handed the user's library.
    let library_source =
        "VAR_GLOBAL CONSTANT PI : LREAL := 3.1415926535897932384626433832795; END_VAR";

    let options = CompilerOptions::default();

    // Rendering the user's library round-trips its source: the *use* of `PI` is
    // preserved...
    let rendered_user = render(user_source, &options);
    assert!(
        rendered_user.contains("PI"),
        "the user's use of PI must be preserved:\n{rendered_user}"
    );
    // ...but the injected library *declaration* is never emitted as user source.
    assert!(
        !rendered_user.contains("VAR_GLOBAL"),
        "an injected VAR_GLOBAL declaration must not be rendered:\n{rendered_user}"
    );
    assert!(
        !rendered_user.contains("3.1415926535897932384626433832795"),
        "the injected PI constant value must not be rendered:\n{rendered_user}"
    );

    // The exclusion is a property of *what `plc2plc` is handed*, not an
    // inability to render the declaration: handed the library, it renders the
    // constant faithfully.
    let rendered_library = render(library_source, &options);
    assert!(
        rendered_library.contains("PI") && rendered_library.contains("VAR_GLOBAL"),
        "the library declaration must render when it is the input:\n{rendered_library}"
    );
}
