//! End-to-end integration tests for the FIND standard function.

use ironplc_parser::options::{CompilerOptions, Dialect};

// 'World' starts at position 7 (1-based).
e2e_i32!(
    end_to_end_when_find_substring_then_returns_position,
    "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'World';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 7)],
);

e2e_i32!(
    end_to_end_when_find_not_found_then_returns_zero,
    "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'XYZ';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 0)],
);

e2e_i32!(
    end_to_end_when_find_at_start_then_returns_one,
    "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'H';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_find_empty_search_then_returns_zero,
    "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING;
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 0)],
);

e2e_i32!(
    end_to_end_when_find_exact_match_then_returns_one,
    "
PROGRAM main
  VAR
    s1 : STRING := 'abc';
    s2 : STRING := 'abc';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_find_search_longer_than_haystack_then_returns_zero,
    "
PROGRAM main
  VAR
    s1 : STRING := 'Hi';
    s2 : STRING := 'Hello';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 0)],
);

// 'DE' starts at position 4 (1-based).
e2e_i32!(
    end_to_end_when_find_at_end_then_returns_correct_position,
    "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'DE';
    n : INT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    &[(2, 4)],
);

// MID('world', L=3, P=1) = 'wor', FIND('hello world', 'wor') = 7 (1-based).
e2e_i32!(
    end_to_end_when_find_with_nested_mid_then_returns_position,
    "
PROGRAM main
  VAR
    s1 : STRING := 'hello world';
    s2 : STRING := 'world';
    n : INT;
  END_VAR
  n := FIND(s1, MID(s2, 3, 1));
END_PROGRAM
",
    &[(2, 7)],
);

// 'bet' starts at position 1 in 'beta'.
// Rusty dialect: var 0-1 system, var 2 struct, var 3 scratch, var 4 pos.
e2e_i32_with!(
    end_to_end_when_find_with_struct_array_field_then_returns_position,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "
TYPE MY_SETUP :
  STRUCT
    NAMES : ARRAY[1..3] OF STRING[20];
  END_STRUCT;
END_TYPE

VAR_GLOBAL
    setup : MY_SETUP;
END_VAR

PROGRAM main
VAR
    pos : INT;
END_VAR
    setup.NAMES[1] := 'alpha';
    setup.NAMES[2] := 'beta';
    pos := FIND(setup.NAMES[2], 'bet');
END_PROGRAM
",
    &[(4, 1)],
);
