//! End-to-end integration tests for function forms of operators.
//!
//! One smoke test per function form to verify the full pipeline works.
//! Detailed opcode testing is in compile_func_forms.rs.
//!
//! Note: NOT(x) is tested via the unary operator path since the parser
//! treats NOT as a unary operator applied to parenthesized expression (x).

mod common;

use common::parse_and_run;

// --- Arithmetic functions ---

#[test]
fn end_to_end_when_add_function_then_returns_sum() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := ADD(x, 32);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_sub_function_then_returns_difference() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := SUB(10, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
}

#[test]
fn end_to_end_when_mul_function_then_returns_product() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := MUL(6, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_div_function_then_returns_quotient() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := DIV(20, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 5);
}

// --- Comparison functions ---

#[test]
fn end_to_end_when_gt_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := GT(10, 5);
  false_result := GT(5, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_ge_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    equal_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := GE(10, 5);
  equal_result := GE(5, 5);
  false_result := GE(5, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 1);
    assert_eq!(bufs.vars[2].as_i32(), 0);
}

#[test]
fn end_to_end_when_eq_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := EQ(5, 5);
  false_result := EQ(5, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_le_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := LE(5, 10);
  false_result := LE(10, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_lt_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := LT(5, 10);
  false_result := LT(5, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_ne_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := NE(5, 10);
  false_result := NE(5, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_mod_function_then_returns_remainder() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := MOD(10, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_and_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := AND(TRUE, TRUE);
  false_result := AND(TRUE, FALSE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_or_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := OR(FALSE, TRUE);
  false_result := OR(FALSE, FALSE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_xor_function_then_returns_bool() {
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := XOR(TRUE, FALSE);
  false_result := XOR(TRUE, TRUE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_not_parens_then_returns_negation() {
    // NOT(x) parses as unary NOT applied to parenthesized expression (x).
    // This is semantically equivalent to the NOT function form.
    let source = "
PROGRAM main
  VAR
    true_result : DINT;
    false_result : DINT;
  END_VAR
  true_result := NOT(FALSE);
  false_result := NOT(TRUE);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}
