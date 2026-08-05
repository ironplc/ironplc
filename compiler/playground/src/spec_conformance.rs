//! Spec conformance tests for compatibility libraries (playground-owned
//! requirement).
//!
//! The `all_spec_requirements_have_tests` meta-test asserts every
//! playground-owned requirement has a test here.
//!
//! See `specs/design/compatibility-libraries.md`.

use serde_json::Value;
use spec_test_macro::spec_test;

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

/// A user POU that uses `PI`, the constant `Tc2_System` provides. Standing
/// alone the name is undefined; it resolves only when the library is activated.
const PI_PROGRAM: &str = "PROGRAM main
VAR
    d2r : LREAL := PI / 180.0;
END_VAR
END_PROGRAM";

/// The plain-text `Tc2_System` library file, as served alongside the app and
/// fetched by the browser. Injecting it activates the library.
const TC2_SYSTEM_ST: &str =
    "VAR_GLOBAL CONSTANT PI : LREAL := 3.1415926535897932384626433832795; END_VAR";

fn compile_ok(source: &str, libraries: &str) -> bool {
    let json = crate::compile(source, "", "", libraries);
    let value: Value = serde_json::from_str(&json).unwrap();
    value["ok"] == Value::Bool(true)
}

/// REQ-CL-playground-001: The playground activates a library by loading it from
/// the plain-text library files served alongside the app.
#[spec_test(REQ_CL_playground_001)]
fn playground_spec_req_cl_001_activates_library_from_served_files() {
    // With no library activated, `PI` is undefined and the compile fails.
    assert!(
        !compile_ok(PI_PROGRAM, ""),
        "PI must be undefined when no library is loaded"
    );

    // Loading the served library file (as the browser would, passing its text
    // as a JSON array of sources) activates the library: `PI` resolves, folds
    // at compile time, and the same source compiles.
    let libraries = serde_json::to_string(&[TC2_SYSTEM_ST]).unwrap();
    assert!(
        compile_ok(PI_PROGRAM, &libraries),
        "loading the served library file must make PI resolve"
    );
}
