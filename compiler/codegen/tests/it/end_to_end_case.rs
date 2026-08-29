//! End-to-end integration tests for CASE statement compilation.

use ironplc_parser::options::CompilerOptions;

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
    &[(0, 1), (1, 10)],
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
    &[(0, 2), (1, 20)],
);

// vars[1] (y) is untouched when there is no match and no ELSE.
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
    &[(0, 99), (1, 0)],
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
    &[(0, 99), (1, 99)],
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
    &[(0, 3), (1, 30)],
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
    &[(0, 3), (1, 50)],
);

/// Options enabling only the bit-string CASE label extension, on the
/// default Edition 2 base. The selector is a standard integer type (`DINT`);
/// only the radix-prefixed *label* form is the extension under test.
fn opts_with_bit_string_case_labels() -> CompilerOptions {
    CompilerOptions {
        allow_bit_string_case_labels: true,
        ..CompilerOptions::default()
    }
}

// Real motivating shape: a private test corpus file uses radix-prefixed
// bit-string literals (16#D012:) as CASE labels.
// The selector is assigned the decimal equivalent (16#D012 == 53266) so
// that everything but the label form stays standard.
e2e_i32_with!(
    end_to_end_when_case_label_is_hex_literal_then_matches_correct_arm,
    opts_with_bit_string_case_labels(),
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
    &[(1, 1)],
);

e2e_i32_with!(
    end_to_end_when_case_label_is_binary_literal_then_matches_correct_arm,
    opts_with_bit_string_case_labels(),
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
    &[(1, 2)],
);

e2e_i32_with!(
    end_to_end_when_case_label_is_hex_literal_and_no_match_then_no_arm_executes,
    opts_with_bit_string_case_labels(),
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
    &[(1, 99)],
);
