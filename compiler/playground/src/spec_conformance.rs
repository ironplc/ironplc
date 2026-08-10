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

/// The `Tc2_Math`-shaped package payload: manifest with an intrinsic binding
/// plus the version's `.st` declaration, as `app.ts` now sends them
/// (`{ manifest, files }`).
#[test]
fn compile_when_package_payload_with_intrinsic_binding_then_bound_call_compiles() {
    let manifest = "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n[\"1.0.0\".bindings]\nLTRUNC = { intrinsic = \"trunc_lreal\" }\n";
    let declaration =
        "FUNCTION LTRUNC : LREAL\nVAR_INPUT\n    IN : LREAL;\nEND_VAR\n;\nEND_FUNCTION\n";
    let libraries = serde_json::json!([{ "manifest": manifest, "files": [declaration] }]);
    let program = "PROGRAM main
VAR
    x : LREAL;
END_VAR
    x := LTRUNC(3.7);
END_PROGRAM";

    assert!(
        compile_ok(program, &libraries.to_string()),
        "an intrinsic-bound call must compile when the package payload carries the manifest"
    );
    // Without the library, the same call must not compile.
    assert!(!compile_ok(program, ""));
}

/// A declare-only binding in the package payload fails compilation of a call
/// with P4046 (never wrong codegen), matching the CLI behavior.
#[test]
fn compile_when_package_payload_with_declare_only_then_call_fails_p4046() {
    let manifest = "name = \"Tc2_Utilities\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n[\"1.0.0\".bindings]\nLREAL_TO_FMTSTR = \"declare-only\"\n";
    let declaration = "FUNCTION LREAL_TO_FMTSTR : STRING[255]\nVAR_INPUT\n    in : LREAL;\n    iPrecision : INT;\n    bRound : BOOL;\nEND_VAR\n;\nEND_FUNCTION\n";
    let libraries = serde_json::json!([{ "manifest": manifest, "files": [declaration] }]);
    let program = "PROGRAM main
VAR
    s : STRING[255];
END_VAR
    s := LREAL_TO_FMTSTR(1.5, 2, TRUE);
END_PROGRAM";

    let json = crate::compile(program, "", "", &libraries.to_string());
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["ok"], Value::Bool(false));
    let codes: Vec<&str> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert_eq!(codes, ["P4046"]);
}
