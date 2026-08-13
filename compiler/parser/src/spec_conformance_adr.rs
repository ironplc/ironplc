//! Spec conformance tests for the TwinCAT/CODESYS `ADR()` operator
//! (parser-owned requirements): dialect presets for the `allow_adr` flag.
//!
//! `ADR` itself needs no parser change — it parses as an ordinary function
//! call — so the parser owns only the options/dialect requirements. Each test
//! is annotated with `#[spec_test(REQ_PTR_parser_NNN)]`, which adds `#[test]`
//! and references a build-script-generated constant so the test fails to
//! compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every parser-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use spec_test_macro::spec_test;

use crate::options::{CompilerOptions, Dialect};

/// REQ-PTR-parser-400: The `codesys` dialect enables `allow_adr`.
#[spec_test(REQ_PTR_parser_400)]
fn options_spec_req_ptr_400_codesys_enables_adr() {
    assert!(CompilerOptions::from_dialect(Dialect::Codesys).allow_adr);
}

/// REQ-PTR-parser-401: The `twincat` dialect enables `allow_adr`.
#[spec_test(REQ_PTR_parser_401)]
fn options_spec_req_ptr_401_twincat_enables_adr() {
    assert!(CompilerOptions::from_dialect(Dialect::TwinCat).allow_adr);
}

/// REQ-PTR-parser-402: The `iec61131-3-ed2`, `iec61131-3-ed3`, and `rusty`
/// dialects do not enable `allow_adr`.
#[spec_test(REQ_PTR_parser_402)]
fn options_spec_req_ptr_402_other_dialects_do_not_enable_adr() {
    assert!(!CompilerOptions::from_dialect(Dialect::Iec61131_3Ed2).allow_adr);
    assert!(!CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3).allow_adr);
    assert!(!CompilerOptions::from_dialect(Dialect::Rusty).allow_adr);
}
