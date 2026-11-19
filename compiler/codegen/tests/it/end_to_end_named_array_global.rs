//! End-to-end tests for `VAR_GLOBAL` variables declared with a named array
//! type (`TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE` ... `g : A3;`).
//!
//! The global grammar spells such a declaration differently from a `VAR`
//! block; these tests check it is laid out like any other array, whether a
//! program reads it, writes it or keeps it across scans.

use ironplc_parser::options::{CompilerOptions, Dialect};

use crate::common::run_scans;

e2e_i32!(
    end_to_end_when_configuration_global_of_named_array_type_then_elements_read_and_written,
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    g : A3;
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
  g[1] := 11;
  g[3] := 33;
  first := g[1];
  third := g[3];
END_PROGRAM
",
    &[("first", 11), ("third", 33)],
);

// Each scan adds 5 to g[2], so the element must keep its value between scans.
#[test]
fn end_to_end_when_configuration_global_of_named_array_type_then_elements_persist_across_scans() {
    let source = "
TYPE A3 : ARRAY[1..3] OF INT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    g : A3;
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
    seen : INT;
  END_VAR
  g[2] := g[2] + 5;
  seen := g[2];
END_PROGRAM
";
    // The task runs every 100 ms, so the second scan runs 100 ms later.
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("seen"), 5);

        session.scan(100_000).unwrap();
        assert_eq!(session.read("seen"), 10);
    });
}

e2e_i32!(
    end_to_end_when_configuration_global_of_named_two_dimensional_array_type_then_indexed_row_major,
    "
TYPE GRID : ARRAY[1..2, 1..3] OF INT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    grid : GRID;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    grid : GRID;
  END_VAR
  VAR
    result : INT;
  END_VAR
  grid[1, 3] := 13;
  grid[2, 1] := 21;
  result := grid[1, 3] * 100 + grid[2, 1];
END_PROGRAM
",
    &[("result", 1321)],
);

e2e_i32!(
    end_to_end_when_configuration_global_of_named_array_of_structures_then_fields_read_and_written,
    "
TYPE
  Item : STRUCT
    a : DINT;
    b : DINT;
  END_STRUCT;
  Items : ARRAY[1..3] OF Item;
END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    devices : Items;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    devices : Items;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  devices[1].a := 100;
  devices[3].b := 200;
  result := devices[1].a + devices[3].b;
END_PROGRAM
",
    &[("result", 300)],
);

e2e_i32!(
    end_to_end_when_configuration_global_of_named_string_array_type_then_elements_read_and_written,
    "
TYPE NAMES : ARRAY[1..2] OF STRING[8]; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    names : NAMES;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    names : NAMES;
  END_VAR
  VAR
    size : INT;
  END_VAR
  names[2] := 'abc';
  size := LEN(names[2]);
END_PROGRAM
",
    &[("size", 3)],
);

e2e_i32_with!(
    end_to_end_when_top_level_global_of_named_array_type_then_elements_read_and_written,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "
TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE

VAR_GLOBAL
  g : A3;
END_VAR

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  g[1] := 40;
  g[2] := 2;
  result := g[1] + g[2];
END_PROGRAM
",
    &[("result", 42)],
);
