//! End-to-end tests for an array initializer written after the name of an
//! array type: `TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE` ... `x : A3 := [1, 2,
//! 3];`.
//!
//! Each test copies elements into scalar program variables and reads those
//! after the first scan, so it checks the values the declaration starts
//! with, in each position that takes one.

use ironplc_parser::options::CompilerOptions;

use crate::common::run_scans;

e2e_i32!(
    end_to_end_when_local_of_named_array_with_values_then_elements_start_with_values,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

PROGRAM main
  VAR
    x : A3 := [1, 2, 3];
    first : DINT;
    second : DINT;
    third : DINT;
  END_VAR
  first := x[1];
  second := x[2];
  third := x[3];
END_PROGRAM
",
    &[("first", 1), ("second", 2), ("third", 3)],
);

e2e_i32!(
    end_to_end_when_named_array_with_fewer_values_than_elements_then_rest_start_at_zero,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

PROGRAM main
  VAR
    x : A3 := [5];
    first : DINT;
    third : DINT;
  END_VAR
  first := x[1];
  third := x[3];
END_PROGRAM
",
    &[("first", 5), ("third", 0)],
);

e2e_i32!(
    end_to_end_when_program_input_of_named_array_with_repeated_value_then_value_repeated,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

PROGRAM main
  VAR_INPUT
    i : A3 := [2(7), 1];
  END_VAR
  VAR
    first : DINT;
    second : DINT;
    third : DINT;
  END_VAR
  first := i[1];
  second := i[2];
  third := i[3];
END_PROGRAM
",
    &[("first", 7), ("second", 7), ("third", 1)],
);

e2e_i32!(
    end_to_end_when_configuration_global_of_named_array_with_values_then_program_reads_values,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    g : A3 := [4, 5, 6];
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    g : A3;
  END_VAR
  VAR
    first : DINT;
    third : DINT;
  END_VAR
  first := g[1];
  third := g[3];
END_PROGRAM
",
    &[("first", 4), ("third", 6)],
);

e2e_i32!(
    end_to_end_when_located_named_array_with_values_then_elements_start_with_values,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

PROGRAM main
  VAR
    x AT %MD10 : A3 := [4, 5, 6];
  END_VAR
  VAR
    third : DINT;
  END_VAR
  third := x[3];
END_PROGRAM
",
    &[("third", 6)],
);

e2e_i32!(
    end_to_end_when_named_two_dimensional_array_with_values_then_values_fill_row_major,
    "
TYPE GRID : ARRAY[1..2, 1..2] OF INT; END_TYPE

PROGRAM main
  VAR
    grid : GRID := [11, 12, 21, 22];
    result : INT;
  END_VAR
  result := grid[1, 2] * 100 + grid[2, 1];
END_PROGRAM
",
    &[("result", 1221)],
);

e2e_i32!(
    end_to_end_when_named_string_array_with_values_then_elements_hold_strings,
    "
TYPE NAMES : ARRAY[1..2] OF STRING[8]; END_TYPE

PROGRAM main
  VAR
    names : NAMES := ['ab', 'cde'];
    size : INT;
  END_VAR
  size := LEN(names[1]) * 10 + LEN(names[2]);
END_PROGRAM
",
    &[("size", 23)],
);

e2e_i32!(
    end_to_end_when_named_enumeration_array_with_values_then_elements_hold_values,
    "
TYPE
  COLOR : (Red, Green, Blue);
  COLORS : ARRAY[1..2] OF COLOR;
END_TYPE

PROGRAM main
  VAR
    colors : COLORS := [Green, Blue];
    first_is_green : BOOL;
    second_is_blue : BOOL;
  END_VAR
  first_is_green := colors[1] = Green;
  second_is_blue := colors[2] = Blue;
END_PROGRAM
",
    &[("first_is_green", 1), ("second_is_blue", 1)],
);

e2e_i32!(
    end_to_end_when_alias_of_named_array_with_values_then_elements_start_with_values,
    "
TYPE
  A3 : ARRAY[1..3] OF DINT;
  B3 : A3;
END_TYPE

PROGRAM main
  VAR
    b : B3 := [7, 8, 9];
    third : DINT;
  END_VAR
  third := b[3];
END_PROGRAM
",
    &[("third", 9)],
);

e2e_i32!(
    end_to_end_when_type_alias_of_named_array_has_values_then_variables_start_with_values,
    "
TYPE
  A3 : ARRAY[1..3] OF DINT;
  B3 : A3 := [7, 8, 9];
END_TYPE

PROGRAM main
  VAR
    b : B3;
    second : DINT;
  END_VAR
  second := b[2];
END_PROGRAM
",
    &[("second", 8)],
);

e2e_i32!(
    end_to_end_when_structure_field_of_named_array_has_values_then_field_starts_with_values,
    "
TYPE
  A3 : ARRAY[1..3] OF DINT;
  S : STRUCT
    f : A3 := [1, 2, 3];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : S;
    second : DINT;
  END_VAR
  second := s.f[2];
END_PROGRAM
",
    &[("second", 2)],
);

// Each scan adds 5 to x[2]: the initial value is applied once, before the
// first scan, and a write replaces it.
#[test]
fn end_to_end_when_named_array_with_values_written_then_written_value_persists_across_scans() {
    let source = "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR
    x : A3 := [1, 2, 3];
    seen : DINT;
  END_VAR
  x[2] := x[2] + 5;
  seen := x[2];
END_PROGRAM
";
    // The task runs every 100 ms, so the second scan runs 100 ms later.
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("seen"), 7);

        session.scan(100_000).unwrap();
        assert_eq!(session.read("seen"), 12);
    });
}
