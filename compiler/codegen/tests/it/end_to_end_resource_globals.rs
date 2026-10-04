//! End-to-end integration tests for global variables declared in a RESOURCE.
//!
//! A `VAR_GLOBAL` inside a `RESOURCE` is visible to the programs of that
//! resource (IEC 61131-3, 2.7.1). These tests compile such a program and run
//! it in the VM, reading and writing the global through `VAR_EXTERNAL`.
//!
//! Globals take the first variable-table slots: configuration globals first,
//! then the globals of the program's resource, then program locals.

use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run_rounds;

// count is at index 0 (resource global)
e2e_i32!(
    end_to_end_when_resource_global_written_via_external_then_value_updated,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count : INT;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    count : INT;
  END_VAR
  count := count + 1;
END_PROGRAM
",
    &[(0, 1)],
);

// limit is at index 0 (resource global), result at index 1 (program local)
e2e_i32!(
    end_to_end_when_resource_global_has_initial_value_then_external_reads_value,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      limit : DINT := 250;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    limit : DINT;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  result := limit;
END_PROGRAM
",
    &[(0, 250), (1, 250)],
);

// base is at index 0 (configuration global), offset at index 1 (resource
// global), result at index 2 (program local)
e2e_i32!(
    end_to_end_when_configuration_and_resource_globals_then_both_readable,
    "
CONFIGURATION config
  VAR_GLOBAL
    base : INT := 100;
  END_VAR
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      offset : INT := 7;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    base : INT;
    offset : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := base + offset;
END_PROGRAM
",
    &[(0, 100), (1, 7), (2, 107)],
);

// readings is at index 0 (resource global), total at index 1 (program local)
e2e_i32!(
    end_to_end_when_resource_global_array_then_elements_readable_and_writable,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      readings : ARRAY[1..3] OF INT := [10, 20, 30];
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    readings : ARRAY[1..3] OF INT;
  END_VAR
  VAR
    total : INT;
  END_VAR
  readings[2] := readings[2] + 5;
  total := readings[1] + readings[2] + readings[3];
END_PROGRAM
",
    &[(1, 65)],
);

// point is at index 0 (resource global), sum at index 1 (program local)
e2e_i32!(
    end_to_end_when_resource_global_struct_then_fields_readable_and_writable,
    "
TYPE POINT :
  STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      point : POINT;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    point : POINT;
  END_VAR
  VAR
    sum : DINT;
  END_VAR
  point.x := 3;
  point.y := 4;
  sum := point.x + point.y;
END_PROGRAM
",
    &[(1, 7)],
);

#[test]
fn end_to_end_when_resource_global_incremented_over_scans_then_value_persists() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count : DINT := 10;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        for (round, expected) in (0..3).zip(11..) {
            vm.run_round(round * 100_000).unwrap();
            assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), expected);
        }
    });
}
