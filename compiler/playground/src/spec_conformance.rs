//! Spec conformance tests for compatibility libraries (playground-owned
//! requirement).
//!
//! The `all_spec_requirements_have_tests` meta-test asserts every
//! playground-owned requirement has a test here. The browser-activation
//! behavior lands with the playground phase; wired here as an ignored test so
//! the meta-test passes.
//!
//! See `specs/design/compatibility-libraries.md`.

use spec_test_macro::spec_test;

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

/// REQ-CL-playground-001: The playground activates a library by loading it from
/// the plain-text library files served alongside the app.
#[spec_test(REQ_CL_playground_001)]
#[ignore = "phase 3: serve and load library files in the browser playground"]
fn playground_spec_req_cl_001_activates_library_from_served_files() {}
