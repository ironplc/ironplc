use crate::test_helpers::{edition3_options, parse_and_resolve_types_with_options};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

fn ref_arithmetic_options() -> CompilerOptions {
    let mut options = edition3_options();
    options.allow_ref_arithmetic = true;
    options
}

/// The problem codes this rule reports for `program` under `options`.
fn problems(program: &str, options: &CompilerOptions) -> Vec<String> {
    let (library, context) = parse_and_resolve_types_with_options(program, options);
    match super::apply(&library, &context, options) {
        Ok(()) => vec![],
        Err(diagnostics) => diagnostics.into_iter().map(|d| d.code).collect(),
    }
}

fn assert_ok(program: &str) {
    let codes = problems(program, &edition3_options());
    assert!(codes.is_empty(), "{codes:?}");
}

fn assert_problem(program: &str, problem: Problem) {
    assert_eq!(problems(program, &edition3_options()), [problem.code()]);
}

// P2036: No nested REF_TO
#[test]
fn ref_to_when_single_level_then_ok() {
    assert_ok(
        "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main
VAR
x : INT;
r : IntRef;
END_VAR
r := REF(x);
END_PROGRAM",
    );
}

// P2028: REF() operand must be a simple variable
#[test]
fn ref_when_operand_is_named_variable_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
    );
}

// P2029: No REF of ephemeral variables - VAR_TEMP
#[test]
fn ref_when_operand_is_var_temp_then_error() {
    assert_problem(
        "FUNCTION_BLOCK FB1
VAR_TEMP
temp : INT;
END_VAR
VAR
r : REF_TO INT;
END_VAR
r := REF(temp);
END_FUNCTION_BLOCK",
        Problem::RefOfEphemeralVariable,
    );
}

// P2029: No REF of FUNCTION VAR_INPUT
#[test]
fn ref_when_operand_is_function_var_input_then_error() {
    assert_problem(
        "FUNCTION MyFunc : INT
VAR_INPUT
inVal : INT;
END_VAR
VAR
r : REF_TO INT;
END_VAR
r := REF(inVal);
MyFunc := 0;
END_FUNCTION",
        Problem::RefOfEphemeralVariable,
    );
}

// P2029: FB VAR_INPUT is persistent — OK
#[test]
fn ref_when_operand_is_fb_var_input_then_ok() {
    assert_ok(
        "FUNCTION_BLOCK FB1
VAR_INPUT
inVal : INT;
END_VAR
VAR
r : REF_TO INT;
END_VAR
r := REF(inVal);
END_FUNCTION_BLOCK",
    );
}

// P2030: No REF of array elements
#[test]
fn ref_when_operand_is_array_element_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
arr : ARRAY [0..9] OF INT;
r : REF_TO INT;
END_VAR
r := REF(arr[3]);
END_PROGRAM",
        Problem::RefOfArrayElement,
    );
}

// P2031: Deref requires reference type
#[test]
fn deref_when_type_is_not_reference_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
x : INT := 42;
y : INT;
END_VAR
y := x^;
END_PROGRAM",
        Problem::DerefRequiresReferenceType,
    );
}

#[test]
fn deref_when_type_is_reference_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT := REF(x);
y : INT;
END_VAR
y := r^;
END_PROGRAM",
    );
}

// P2033: No arithmetic on references
#[test]
fn arithmetic_when_operand_is_reference_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT := REF(x);
y : INT;
END_VAR
y := r + 1;
END_PROGRAM",
        Problem::ArithmeticOnReference,
    );
}

// P2034: NULL only for reference types
#[test]
fn null_when_assigned_to_non_reference_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
x : INT;
END_VAR
x := NULL;
END_PROGRAM",
        Problem::NullRequiresReferenceType,
    );
}

#[test]
fn null_when_assigned_to_reference_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT := REF(x);
END_VAR
r := NULL;
END_PROGRAM",
    );
}

// P2035: Only = and <> on references
#[test]
fn compare_when_equality_on_reference_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
x : INT;
r1 : REF_TO INT := REF(x);
r2 : REF_TO INT := REF(x);
result : BOOL;
END_VAR
result := r1 = r2;
END_PROGRAM",
    );
}

#[test]
fn compare_when_ordering_on_reference_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
x : INT;
r1 : REF_TO INT := REF(x);
r2 : REF_TO INT := REF(x);
result : BOOL;
END_VAR
result := r1 > r2;
END_PROGRAM",
        Problem::OrderingOnReference,
    );
}

// P2032: Reference type mismatch
#[test]
fn assign_when_ref_types_match_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
    );
}

#[test]
fn assign_when_ref_types_incompatible_then_error() {
    assert_problem(
        "PROGRAM Main
VAR
x : REAL;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
        Problem::ReferenceTypeMismatch,
    );
}

#[test]
fn array_of_ref_to_when_declared_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
data : ARRAY[0..3] OF REF_TO BYTE;
END_VAR
END_PROGRAM",
    );
}

#[test]
fn ref_to_array_when_declared_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
data : REF_TO ARRAY[1..10] OF INT;
END_VAR
END_PROGRAM",
    );
}

#[test]
fn ref_to_array_type_decl_when_declared_then_ok() {
    assert_ok(
        "TYPE ArrRef : REF_TO ARRAY[0..3] OF BYTE; END_TYPE
PROGRAM Main
VAR
data : ArrRef;
END_VAR
END_PROGRAM",
    );
}

// --allow-ref-arithmetic tests: negative (flag not set)
#[test]
fn arithmetic_when_ref_arithmetic_not_allowed_then_error() {
    let codes = problems(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT := REF(x);
y : INT;
END_VAR
y := r + 1;
END_PROGRAM",
        &edition3_options(),
    );
    assert_eq!(codes, [Problem::ArithmeticOnReference.code()]);
}

#[test]
fn compare_when_ordering_without_ref_arithmetic_then_error() {
    let codes = problems(
        "PROGRAM Main
VAR
x : INT;
r1 : REF_TO INT := REF(x);
r2 : REF_TO INT := REF(x);
result : BOOL;
END_VAR
result := r1 > r2;
END_PROGRAM",
        &edition3_options(),
    );
    assert_eq!(codes, [Problem::OrderingOnReference.code()]);
}

// --allow-ref-arithmetic tests: positive (flag set)
#[test]
fn arithmetic_when_ref_arithmetic_allowed_then_ok() {
    let codes = problems(
        "PROGRAM Main
VAR
x : INT;
r : REF_TO INT := REF(x);
y : INT;
END_VAR
y := r + 1;
END_PROGRAM",
        &ref_arithmetic_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn compare_when_ordering_with_ref_arithmetic_allowed_then_ok() {
    let codes = problems(
        "PROGRAM Main
VAR
x : INT;
r1 : REF_TO INT := REF(x);
r2 : REF_TO INT := REF(x);
result : BOOL;
END_VAR
result := r1 > r2;
END_PROGRAM",
        &ref_arithmetic_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn compare_when_equality_with_ref_arithmetic_allowed_then_ok() {
    let codes = problems(
        "PROGRAM Main
VAR
x : INT;
r1 : REF_TO INT := REF(x);
r2 : REF_TO INT := REF(x);
result : BOOL;
END_VAR
result := r1 = r2;
END_PROGRAM",
        &ref_arithmetic_options(),
    );
    assert!(codes.is_empty(), "{codes:?}");
}

// P2029: allow_ref_stack_variables suppresses REF of FUNCTION VAR_INPUT
#[test]
fn ref_when_allow_ref_stack_variables_and_function_var_input_then_ok() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_ref_stack_variables: true,
        ..CompilerOptions::default()
    };
    let codes = problems(
        "FUNCTION MyFunc : INT
VAR_INPUT
inVal : INT;
END_VAR
VAR
r : REF_TO INT;
END_VAR
r := REF(inVal);
MyFunc := 0;
END_FUNCTION",
        &options,
    );
    assert!(codes.is_empty(), "{codes:?}");
}

// P2029: allow_ref_stack_variables suppresses REF of VAR_TEMP
#[test]
fn ref_when_allow_ref_stack_variables_and_var_temp_then_ok() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_ref_stack_variables: true,
        ..CompilerOptions::default()
    };
    let codes = problems(
        "FUNCTION_BLOCK FB1
VAR_TEMP
temp : INT;
END_VAR
VAR
r : REF_TO INT;
END_VAR
r := REF(temp);
END_FUNCTION_BLOCK",
        &options,
    );
    assert!(codes.is_empty(), "{codes:?}");
}

// P2032: allow_ref_type_punning suppresses type mismatch
#[test]
fn assign_when_allow_ref_type_punning_and_types_incompatible_then_ok() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_ref_type_punning: true,
        ..CompilerOptions::default()
    };
    let codes = problems(
        "PROGRAM Main
VAR
x : REAL;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
        &options,
    );
    assert!(codes.is_empty(), "{codes:?}");
}

// P2032: type mismatch still fires without allow_ref_type_punning
#[test]
fn assign_when_no_allow_ref_type_punning_and_types_incompatible_then_error() {
    let codes = problems(
        "PROGRAM Main
VAR
x : REAL;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
        &edition3_options(),
    );
    assert_eq!(codes, [Problem::ReferenceTypeMismatch.code()]);
}

// P2032: allow_ref_stack_variables alone does NOT suppress type mismatch
#[test]
fn assign_when_allow_ref_stack_variables_only_and_types_incompatible_then_error() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_ref_stack_variables: true,
        ..CompilerOptions::default()
    };
    let codes = problems(
        "PROGRAM Main
VAR
x : REAL;
r : REF_TO INT;
END_VAR
r := REF(x);
END_PROGRAM",
        &options,
    );
    assert_eq!(codes, [Problem::ReferenceTypeMismatch.code()]);
}

#[test]
fn ref_when_method_local_reference_then_ok() {
    assert_ok(
        "FUNCTION_BLOCK FB
VAR
x : INT;
END_VAR
METHOD Go
VAR
r : REF_TO INT;
END_VAR
r := REF(x);
r^ := 1;
END_METHOD
END_FUNCTION_BLOCK",
    );
}

#[test]
fn assign_when_method_uses_function_block_reference_field_then_ok() {
    assert_ok(
        "FUNCTION_BLOCK FB
VAR
x : INT;
r : REF_TO INT;
END_VAR
METHOD Go
r := REF(x);
r := NULL;
END_METHOD
END_FUNCTION_BLOCK",
    );
}

#[test]
fn assign_when_method_local_shadows_field_then_uses_local() {
    assert_ok(
        "FUNCTION_BLOCK FB
VAR
x : INT;
r : INT;
END_VAR
METHOD Go
VAR
r : REF_TO INT;
END_VAR
r := NULL;
END_METHOD
END_FUNCTION_BLOCK",
    );
}

#[test]
fn assign_when_sibling_method_declares_reference_then_not_visible() {
    assert_problem(
        "FUNCTION_BLOCK FB
VAR
r : INT;
END_VAR
METHOD A
VAR
r : REF_TO INT;
END_VAR
END_METHOD
METHOD B
r := NULL;
END_METHOD
END_FUNCTION_BLOCK",
        Problem::NullRequiresReferenceType,
    );
}

#[test]
fn assign_when_named_reference_type_targets_other_type_then_error() {
    assert_problem(
        "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main
VAR
    y : REAL;
    r : IntRef;
END_VAR
    r := REF(y);
END_PROGRAM",
        Problem::ReferenceTypeMismatch,
    );
}

#[test]
fn assign_when_reference_to_inline_array_then_ok() {
    assert_ok(
        "PROGRAM Main
VAR
    a : ARRAY[1..2] OF INT;
    r : REF_TO ARRAY[1..2] OF INT;
END_VAR
    r := REF(a);
END_PROGRAM",
    );
}
