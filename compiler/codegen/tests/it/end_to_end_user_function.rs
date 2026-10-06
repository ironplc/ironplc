//! End-to-end integration tests for user-defined function calls.

use crate::common::Snapshot;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

e2e_i32!(
    end_to_end_when_user_function_add_then_returns_sum,
    "FUNCTION add_two : DINT VAR_INPUT a : DINT; b : DINT; END_VAR add_two := a + b; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := add_two(3, 7); END_PROGRAM",
    &[("result", 10)],
);

e2e_i32!(
    end_to_end_when_user_function_with_local_var_then_correct,
    "FUNCTION double_plus_one : DINT VAR_INPUT x : DINT; END_VAR VAR temp : DINT; END_VAR temp := x * 2; double_plus_one := temp + 1; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := double_plus_one(5); END_PROGRAM",
    &[("result", 11)],
);

e2e_i32!(
    end_to_end_when_user_function_called_twice_then_both_correct,
    "FUNCTION square : DINT VAR_INPUT n : DINT; END_VAR square := n * n; END_FUNCTION PROGRAM main VAR a : DINT; b : DINT; END_VAR a := square(3); b := square(5); END_PROGRAM",
    &[("a", 9), ("b", 25)],
);

e2e_i32!(
    end_to_end_when_user_function_in_expression_then_correct,
    "FUNCTION inc : DINT VAR_INPUT x : DINT; END_VAR inc := x + 1; END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := inc(10) + inc(20); END_PROGRAM",
    &[("result", 32)],
);

// FOO assigns 8, then shifts right by 1: 8 >> 1 = 4.
e2e_i32!(
    end_to_end_when_user_function_assigns_return_var_then_uses_in_builtin_then_correct,
    "FUNCTION FOO : WORD VAR_INPUT A : INT; END_VAR FOO := WORD#8; FOO := SHR(FOO, 1); END_FUNCTION PROGRAM main VAR result : WORD; END_VAR result := FOO(A := 5); END_PROGRAM",
    &[("result", 4)],
);

// The motivating example from ADR-0024: a function with a local variable that
// has an initial value must re-initialize on every call.
// First call: counter starts at 10, adds 5 -> 15.
// Second call: counter must restart at 10 (not 15), adds 3 -> 13.
e2e_i32!(
    end_to_end_when_function_called_twice_then_locals_reinitialized,
    "FUNCTION accumulate : DINT VAR_INPUT a : DINT; END_VAR VAR counter : DINT := 10; END_VAR counter := counter + a; accumulate := counter; END_FUNCTION PROGRAM main VAR r1 : DINT; r2 : DINT; END_VAR r1 := accumulate(5); r2 := accumulate(3); END_PROGRAM",
    &[("r1", 15), ("r2", 13)],
);

// A function with a local variable that has no explicit initializer must be
// zero-initialized on every call.
// First call: accum starts at 0, adds 7 -> 7.
// Second call: accum must restart at 0 (not 7), adds 3 -> 3.
e2e_i32!(
    end_to_end_when_function_called_twice_then_zero_default_locals_reinitialized,
    "FUNCTION sum_via_local : DINT VAR_INPUT x : DINT; END_VAR VAR accum : DINT; END_VAR accum := accum + x; sum_via_local := accum; END_FUNCTION PROGRAM main VAR r1 : DINT; r2 : DINT; END_VAR r1 := sum_via_local(7); r2 := sum_via_local(3); END_PROGRAM",
    &[("r1", 7), ("r2", 3)],
);

// The return variable must also be zero-initialized on every call. If it
// retained its value, the second call would see the first call's result.
// First call: flag > 0, so return 42.
// Second call: flag = 0, so return value stays at default (0), not stale 42.
e2e_i32!(
    end_to_end_when_function_called_twice_then_return_value_reinitialized,
    "FUNCTION conditional_set : DINT VAR_INPUT flag : DINT; END_VAR IF flag > 0 THEN conditional_set := 42; END_IF; END_FUNCTION PROGRAM main VAR r1 : DINT; r2 : DINT; END_VAR r1 := conditional_set(1); r2 := conditional_set(0); END_PROGRAM",
    &[("r1", 42), ("r2", 0)],
);

// OUTER(3) calls INNER(3), which returns 3 * 2 = 6.
e2e_i32!(
    end_to_end_when_user_function_calls_another_user_function_then_correct,
    "FUNCTION INNER : DINT VAR_INPUT X : DINT; END_VAR INNER := X * 2; END_FUNCTION FUNCTION OUTER : DINT VAR_INPUT Y : DINT; END_VAR OUTER := INNER(X := Y); END_FUNCTION PROGRAM main VAR result : DINT; END_VAR result := OUTER(Y := 3); END_PROGRAM",
    &[("result", 6)],
);

#[test]
fn end_to_end_when_unused_function_defined_then_result_unchanged() {
    // Without the unused function, the program computes OUTER(A:=3.0, B:=1.0)
    // which calls INNER(X:=3.0), returning 3.0 * 2.0 = 6.0, then adds 1.0 = 7.0.
    let source_without_unused = "
FUNCTION INNER : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  INNER := X * 2.0;
END_FUNCTION

FUNCTION OUTER : REAL
  VAR_INPUT
    A : REAL;
    B : REAL;
  END_VAR
  OUTER := INNER(X := A) + B;
END_FUNCTION

PROGRAM main
  VAR
    result : REAL;
  END_VAR
  result := OUTER(A := 3.0, B := 1.0);
END_PROGRAM
";

    // The same program but with an unused function that references an
    // undefined function. The compiler should tree-shake UNUSED_FUNC
    // so that it never reaches analysis or codegen.
    let source_with_unused = "
FUNCTION INNER : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  INNER := X * 2.0;
END_FUNCTION

FUNCTION UNUSED_FUNC : REAL
  VAR_INPUT
    X : REAL;
  END_VAR
  UNUSED_FUNC := X + 42.0;
END_FUNCTION

FUNCTION OUTER : REAL
  VAR_INPUT
    A : REAL;
    B : REAL;
  END_VAR
  OUTER := INNER(X := A) + B;
END_FUNCTION

PROGRAM main
  VAR
    result : REAL;
  END_VAR
  result := OUTER(A := 3.0, B := 1.0);
END_PROGRAM
";

    let snapshot1 = Snapshot::run(source_without_unused, &CompilerOptions::default());
    let snapshot2 = Snapshot::run(source_with_unused, &CompilerOptions::default());

    // Both should produce 7.0
    assert_eq!(snapshot1.read_as::<f32>("result"), 7.0);
    assert_eq!(snapshot2.read_as::<f32>("result"), 7.0);
}

e2e_i32!(
    end_to_end_when_user_function_with_string_param_calls_len_then_returns_length,
    "FUNCTION MY_LEN : INT VAR_INPUT S : STRING; END_VAR MY_LEN := LEN(S); END_FUNCTION PROGRAM main VAR result : INT; END_VAR result := MY_LEN(S := 'Hello'); END_PROGRAM",
    &[("result", 5)],
);

// 'Hi there' has 8 characters.
e2e_i32!(
    end_to_end_when_user_function_with_string_param_from_variable_then_correct,
    "FUNCTION MY_LEN : INT VAR_INPUT S : STRING; END_VAR MY_LEN := LEN(S); END_FUNCTION PROGRAM main VAR greeting : STRING := 'Hi there'; result : INT; END_VAR result := MY_LEN(S := greeting); END_PROGRAM",
    &[("result", 8)],
);

e2e_i32!(
    end_to_end_when_user_function_with_string_and_scalar_params_then_correct,
    "FUNCTION CHECK_LEN : INT VAR_INPUT S : STRING; expected : INT; END_VAR VAR actual : INT; END_VAR actual := LEN(S); IF actual = expected THEN CHECK_LEN := 1; ELSE CHECK_LEN := 0; END_IF; END_FUNCTION PROGRAM main VAR result : INT; END_VAR result := CHECK_LEN(S := 'ABC', expected := 3); END_PROGRAM",
    &[("result", 1)],
);

e2e_i32!(
    end_to_end_when_user_function_with_string_param_called_twice_then_both_correct,
    "FUNCTION MY_LEN : INT VAR_INPUT S : STRING; END_VAR MY_LEN := LEN(S); END_FUNCTION PROGRAM main VAR r1 : INT; r2 : INT; END_VAR r1 := MY_LEN(S := 'AB'); r2 := MY_LEN(S := 'ABCDE'); END_PROGRAM",
    &[("r1", 2), ("r2", 5)],
);

// -2.5 < 0.0 is TRUE (1), 2.5 < 0.0 is FALSE (0).
e2e_i32!(
    end_to_end_when_user_function_with_real_comparison_then_correct,
    "FUNCTION SIGN_R : BOOL VAR_INPUT in : REAL; END_VAR SIGN_R := in < 0.0; END_FUNCTION PROGRAM main VAR neg : BOOL; pos : BOOL; END_VAR neg := SIGN_R(in := -2.5); pos := SIGN_R(in := 2.5); END_PROGRAM",
    &[("neg", 1), ("pos", 0)],
);

// The argument is widened to the parameter's width by its own signedness: a
// UDINT above i32::MAX is zero-extended, and a negative DINT sign-extended.
// The arguments are calls, which do not convert themselves to their context,
// so only the recorded conversion widens them.
e2e_i64!(
    #[spec_test(REQ_IC_codegen_001)]
    end_to_end_when_argument_narrower_than_parameter_then_widened_by_its_signedness,
    "FUNCTION widen : LINT VAR_INPUT x : LINT; END_VAR widen := x; END_FUNCTION
     FUNCTION big : UDINT VAR_INPUT x : UDINT; END_VAR big := x; END_FUNCTION
     FUNCTION negative : DINT VAR_INPUT x : DINT; END_VAR negative := x; END_FUNCTION
     PROGRAM main VAR a : LINT; b : LINT; END_VAR
     a := widen(big(4000000000)); b := widen(negative(-5)); END_PROGRAM",
    &[("a", 4_000_000_000), ("b", -5)],
);

e2e_f64!(
    end_to_end_when_real_argument_to_lreal_parameter_then_widened,
    "FUNCTION widen : LREAL VAR_INPUT x : LREAL; END_VAR widen := x; END_FUNCTION
     PROGRAM main VAR r : REAL := 1.5; a : LREAL; END_VAR a := widen(r); END_PROGRAM",
    &[("a", 1.5)],
);

// A literal compiles at the type the analyzer recorded for it: an untyped
// literal at its context's, beyond the range of the default DINT, and a typed
// one at its own, converted to its context's by its own signedness.
e2e_i64!(
    #[spec_test(REQ_IC_codegen_002)]
    end_to_end_when_literal_assigned_then_compiled_at_recorded_type,
    "PROGRAM main VAR a : LINT; b : LINT; END_VAR
     a := 5000000000; b := UDINT#4000000000; END_PROGRAM",
    &[("a", 5_000_000_000), ("b", 4_000_000_000)],
);
