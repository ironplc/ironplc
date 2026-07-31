//! End-to-end integration tests for CASE statement compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

#[test]
fn end_to_end_when_case_matches_first_arm_then_executes_body() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_case_matches_second_arm_then_executes_body() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 2);
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

#[test]
fn end_to_end_when_case_no_match_and_no_else_then_skips() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 99);
    assert_eq!(bufs.vars[1].as_i32(), 0); // untouched
}

#[test]
fn end_to_end_when_case_no_match_with_else_then_executes_else() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 99);
    assert_eq!(bufs.vars[1].as_i32(), 99);
}

#[test]
fn end_to_end_when_case_multi_selector_then_matches_any() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 30);
}

#[test]
fn end_to_end_when_case_subrange_then_matches_in_range() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 3);
    assert_eq!(bufs.vars[1].as_i32(), 50);
}

/// Options enabling only the bit-string CASE label vendor extension, on the
/// default Edition 2 base. The selector is a standard integer type (`DINT`);
/// only the radix-prefixed *label* form is the extension under test.
fn opts_with_bit_string_case_labels() -> CompilerOptions {
    CompilerOptions {
        allow_bit_string_case_labels: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_case_label_is_hex_literal_then_matches_correct_arm() {
    // Real motivating shape: a private test corpus file uses radix-prefixed
    // bit-string literals (16#D012:) as CASE labels. See
    // specs/plans/2026-07-26-twincat-case-label-bit-string-literals.md.
    // The selector is assigned the decimal equivalent (16#D012 == 53266) so
    // that everything but the label form stays standard.
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &opts_with_bit_string_case_labels());

    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_case_label_is_binary_literal_then_matches_correct_arm() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &opts_with_bit_string_case_labels());

    assert_eq!(bufs.vars[1].as_i32(), 2);
}

#[test]
fn end_to_end_when_case_label_is_hex_literal_and_no_match_then_no_arm_executes() {
    let source = "
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
";
    let (_c, bufs) = parse_and_run(source, &opts_with_bit_string_case_labels());

    assert_eq!(bufs.vars[1].as_i32(), 99);
}
