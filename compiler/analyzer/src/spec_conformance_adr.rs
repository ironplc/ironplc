//! Spec conformance tests for the TwinCAT/CODESYS `ADR()` operator
//! (analyzer-owned requirements): the call rewrite and operand diagnostics.
//!
//! Each test is annotated with `#[spec_test(REQ_PTR_analyzer_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test fails
//! to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every analyzer-owned requirement has a test.
//!
//! See `specs/design/adr-and-pointer-to.md`.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;

use crate::stages::analyze;

/// The minimal flag set for `ADR`: the pointer type plus the operator, and
/// deliberately *not* `allow_ref_to` — the `twincat` dialect has no
/// `REF_TO`/`REF()`/`NULL` keywords, so `ADR` must work without them.
fn adr_options() -> CompilerOptions {
    CompilerOptions {
        allow_pointer_to: true,
        allow_adr: true,
        ..CompilerOptions::default()
    }
}

/// Analyze a program and return the problem codes it produced.
fn analyze_codes(program: &str, options: &CompilerOptions) -> Vec<String> {
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (_library, context) = analyze(&[&library], options).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

/// REQ-PTR-analyzer-410: With `allow_adr` on, binding a `POINTER TO T`
/// variable via `ADR` of a `T` variable is accepted — without `allow_ref_to`.
#[spec_test(REQ_PTR_analyzer_410)]
fn analyzer_spec_req_ptr_410_adr_of_variable_is_accepted_without_ref_to() {
    let source = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT;
    y : INT;
END_VAR
    p := ADR(x);
    y := p^;
END_PROGRAM";
    let codes = analyze_codes(source, &adr_options());
    assert!(codes.is_empty(), "expected clean analysis, got {codes:?}");
}

/// REQ-PTR-analyzer-411: With `allow_adr` off, `ADR(x)` is an ordinary
/// identifier and the call is reported as an undeclared function (P4017) —
/// the same fall-through `SIZEOF` has.
#[spec_test(REQ_PTR_analyzer_411)]
fn analyzer_spec_req_ptr_411_adr_is_undeclared_function_when_flag_off() {
    let source = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT;
END_VAR
    p := ADR(x);
END_PROGRAM";
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let codes = analyze_codes(source, &options);
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::FunctionCallUndeclared.code()),
        "expected P4017 (FunctionCallUndeclared), got {codes:?}"
    );
}

/// REQ-PTR-analyzer-412: An `ADR` call with a number of arguments other than
/// one is rejected with P2028.
#[spec_test(REQ_PTR_analyzer_412)]
fn analyzer_spec_req_ptr_412_adr_with_wrong_arity_is_rejected() {
    let source = "PROGRAM Main
VAR
    x : INT;
    y : INT;
    p : POINTER TO INT;
END_VAR
    p := ADR(x, y);
END_PROGRAM";
    let codes = analyze_codes(source, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOperandNotVariable.code()),
        "expected P2028 (RefOperandNotVariable), got {codes:?}"
    );
}

/// REQ-PTR-analyzer-413: An `ADR` operand that is not a variable — a literal
/// or a call result — is rejected with P2028.
#[spec_test(REQ_PTR_analyzer_413)]
fn analyzer_spec_req_ptr_413_adr_of_non_variable_is_rejected() {
    let literal = "PROGRAM Main
VAR
    p : POINTER TO INT;
END_VAR
    p := ADR(5);
END_PROGRAM";
    let codes = analyze_codes(literal, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOperandNotVariable.code()),
        "ADR of a literal: expected P2028, got {codes:?}"
    );

    // A call result is not addressable either — including a nested ADR.
    let call_result = "PROGRAM Main
VAR
    x : INT;
    p : POINTER TO INT;
END_VAR
    p := ADR(ADR(x));
END_PROGRAM";
    let codes = analyze_codes(call_result, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOperandNotVariable.code()),
        "ADR of a call result: expected P2028, got {codes:?}"
    );
}

/// REQ-PTR-analyzer-414: `ADR` of an array element is rejected with P2030 and
/// `ADR` of a structure field with P2028 — slot indices cannot name a
/// sub-object.
#[spec_test(REQ_PTR_analyzer_414)]
fn analyzer_spec_req_ptr_414_adr_of_sub_object_is_rejected() {
    let array_element = "PROGRAM Main
VAR
    arr : ARRAY [0..9] OF INT;
    p : POINTER TO INT;
END_VAR
    p := ADR(arr[3]);
END_PROGRAM";
    let codes = analyze_codes(array_element, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOfArrayElement.code()),
        "ADR of an array element: expected P2030, got {codes:?}"
    );

    let struct_field = "TYPE MyStruct : STRUCT field : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR
    s : MyStruct;
    p : POINTER TO INT;
END_VAR
    p := ADR(s.field);
END_PROGRAM";
    let codes = analyze_codes(struct_field, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOperandNotVariable.code()),
        "ADR of a struct field: expected P2028, got {codes:?}"
    );
}

/// REQ-PTR-analyzer-415: Binding `ADR(x)` to a pointer whose target type
/// differs from `typeof(x)` is rejected with P2032, reusing the `REF_TO`
/// compatibility rule.
#[spec_test(REQ_PTR_analyzer_415)]
fn analyzer_spec_req_ptr_415_adr_type_mismatch_is_rejected() {
    let source = "PROGRAM Main
VAR
    x : REAL;
    p : POINTER TO INT;
END_VAR
    p := ADR(x);
END_PROGRAM";
    let codes = analyze_codes(source, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::ReferenceTypeMismatch.code()),
        "expected P2032 (ReferenceTypeMismatch), got {codes:?}"
    );
}

/// REQ-PTR-analyzer-416: `ADR` of a stack-allocated (`VAR_TEMP`) variable is
/// rejected with P2029 unless `allow_ref_stack_variables` is set.
#[spec_test(REQ_PTR_analyzer_416)]
fn analyzer_spec_req_ptr_416_adr_of_ephemeral_variable_is_rejected() {
    let source = "FUNCTION_BLOCK FB1
VAR_TEMP
    temp : INT;
END_VAR
VAR
    p : POINTER TO INT;
END_VAR
    p := ADR(temp);
END_FUNCTION_BLOCK";
    let codes = analyze_codes(source, &adr_options());
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == Problem::RefOfEphemeralVariable.code()),
        "expected P2029 (RefOfEphemeralVariable), got {codes:?}"
    );
}
