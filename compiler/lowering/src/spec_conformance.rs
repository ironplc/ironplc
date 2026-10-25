//! The completeness check of the requirements this crate owns in
//! `specs/design/execution-model.md`, each marked with the
//! `#[spec_test(REQ_EM_lowering_NNN)]` of its requirement
//! (`execution_model/tests.rs`).

#[test]
fn all_spec_requirements_have_tests() {
    // UNTESTED is computed by build.rs by scanning all .rs files under src/
    // for #[spec_test(REQ_...)] attributes.
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}
