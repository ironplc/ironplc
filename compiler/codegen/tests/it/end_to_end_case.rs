//! End-to-end integration tests for CASE statement compilation.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::assert_run_with;
use crate::compile_body::compile_and_run;

e2e_i32!(
    end_to_end_when_case_matches_first_arm_then_executes_body,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 1;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
",
    &[("x", 1), ("y", 10)],
);

e2e_i32!(
    end_to_end_when_case_matches_second_arm_then_executes_body,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 2;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
",
    &[("x", 2), ("y", 20)],
);

// y is untouched when there is no match and no ELSE.
e2e_i32!(
    end_to_end_when_case_no_match_and_no_else_then_skips,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 99;
  CASE x OF
    1: y := 10;
    2: y := 20;
  END_CASE;
END_PROGRAM
",
    &[("x", 99), ("y", 0)],
);

e2e_i32!(
    end_to_end_when_case_no_match_with_else_then_executes_else,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 99;
  CASE x OF
    1: y := 10;
    2: y := 20;
  ELSE
    y := 99;
  END_CASE;
END_PROGRAM
",
    &[("x", 99), ("y", 99)],
);

e2e_i32!(
    end_to_end_when_case_multi_selector_then_matches_any,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 3;
  CASE x OF
    1: y := 10;
    2, 3: y := 30;
  END_CASE;
END_PROGRAM
",
    &[("x", 3), ("y", 30)],
);

e2e_i32!(
    end_to_end_when_case_subrange_then_matches_in_range,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 3;
  CASE x OF
    1..5: y := 50;
    10: y := 100;
  END_CASE;
END_PROGRAM
",
    &[("x", 3), ("y", 50)],
);

/// Options enabling only the radix CASE label extension, on the
/// default Edition 2 base. The selector is a standard integer type (`DINT`);
/// only the radix-prefixed *label* form is the extension under test.
fn opts_with_radix_case_labels() -> CompilerOptions {
    CompilerOptions {
        allow_radix_case_labels: true,
        ..CompilerOptions::default()
    }
}

// Real motivating shape: a private test corpus file uses radix-prefixed
// bit-string literals (16#D012:) as CASE labels.
// The selector is assigned the decimal equivalent (16#D012 == 53266) so
// that everything but the label form stays standard.
e2e_i32_with!(
    end_to_end_when_case_label_is_hex_literal_then_matches_correct_arm,
    opts_with_radix_case_labels(),
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 53266;
  CASE x OF
    16#D012: y := 1;
    2#1010: y := 2;
  END_CASE;
END_PROGRAM
",
    &[("y", 1)],
);

e2e_i32_with!(
    end_to_end_when_case_label_is_binary_literal_then_matches_correct_arm,
    opts_with_radix_case_labels(),
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  CASE x OF
    16#D012: y := 1;
    2#1010: y := 2;
  END_CASE;
END_PROGRAM
",
    &[("y", 2)],
);

e2e_i32_with!(
    end_to_end_when_case_label_is_hex_literal_and_no_match_then_no_arm_executes,
    opts_with_radix_case_labels(),
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := 99;
  x := 1;
  CASE x OF
    16#D012: y := 1;
  END_CASE;
END_PROGRAM
",
    &[("y", 99)],
);

/// A label is narrowed to the selector's width by its value, whatever radix
/// it was written in, so a value that fills an unsigned selector matches
/// whether it was spelled in decimal or in hex.
#[rstest]
#[case::decimal_udint("UDINT", "4294967295", "4294967295")]
#[case::hex_udint("UDINT", "4294967295", "16#FFFFFFFF")]
#[case::subrange_udint("UDINT", "4294967295", "3000000000..4294967295")]
#[case::decimal_ulint("ULINT", "18446744073709551615", "18446744073709551615")]
#[case::hex_ulint("ULINT", "18446744073709551615", "16#FFFFFFFFFFFFFFFF")]
#[case::subrange_ulint(
    "ULINT",
    "18446744073709551615",
    "10000000000000000000..18446744073709551615"
)]
fn end_to_end_when_case_label_fills_unsigned_selector_then_matches(
    #[case] selector_type: &str,
    #[case] selector_value: &str,
    #[case] label: &str,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    y : DINT;
    x : {selector_type} := {selector_value};
  END_VAR
  CASE x OF
    {label}: y := 1;
  END_CASE;
END_PROGRAM
"
    );

    assert_run_with::<i32>(&source, &opts_with_radix_case_labels(), &[("y", 1)]);
}

/// A label on a signed 64-bit selector is narrowed to the selector's width as
/// a signed value, so one beyond 32 bits, a negative one, and a negative
/// subrange each match the selector value they name.
#[rstest]
#[case::beyond_32_bits("5000000000", "5000000000")]
#[case::negative("-5000000000", "-5000000000")]
#[case::negative_subrange("-5000000000", "-6000000000..-4000000000")]
fn end_to_end_when_case_label_on_lint_selector_then_matches(
    #[case] selector_value: &str,
    #[case] label: &str,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    y : DINT;
    x : LINT := {selector_value};
  END_VAR
  CASE x OF
    {label}: y := 1;
  END_CASE;
END_PROGRAM
"
    );

    assert_run_with::<i32>(&source, &CompilerOptions::default(), &[("y", 1)]);
}

/// REQ-LOW-codegen-124: a selector with a side effect runs once, whichever
/// way through the labels the `CASE` takes: the first label, a label before
/// the last of its arm or the last, a range, a range failing at either
/// bound, with or without an `ELSE`. `NEXT_VAL` counts its calls in
/// `calls`, so `calls` ends one above where it started, and a selector left
/// on the operand stack fails the test.
#[rstest]
#[case::first_label(4, "ELSE y := -1;", 5, 5)]
#[case::label_before_last_of_arm(5, "ELSE y := -1;", 6, 6)]
#[case::last_label_of_arm(6, "ELSE y := -1;", 7, 6)]
#[case::range(8, "ELSE y := -1;", 9, 8)]
#[case::below_range_runs_else(0, "ELSE y := -1;", 1, -1)]
#[case::above_range_runs_else(20, "ELSE y := -1;", 21, -1)]
#[case::no_match_without_else(20, "", 21, 0)]
fn end_to_end_when_case_selector_has_side_effect_then_evaluated_once(
    #[case] start: i64,
    #[case] otherwise: &str,
    #[case] calls: i64,
    #[case] y: i64,
) {
    let snapshot = compile_and_run(&format!(
        "
FUNCTION NEXT_VAL : DINT
  VAR_IN_OUT c : DINT; END_VAR
  c := c + 1;
  NEXT_VAL := c;
END_FUNCTION
PROGRAM main
  VAR
    calls : DINT := {start};
    y : DINT;
  END_VAR
  CASE NEXT_VAL(c := calls) OF
    5: y := 5;
    6, 7: y := 6;
    8..10: y := 8;
  {otherwise}
  END_CASE;
END_PROGRAM
"
    ));

    assert_eq!(snapshot.read_as::<i64>("calls"), calls);
    assert_eq!(snapshot.read_as::<i64>("y"), y);
}
