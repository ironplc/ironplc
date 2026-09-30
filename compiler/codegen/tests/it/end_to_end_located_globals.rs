//! End-to-end integration tests for located global variables
//! (`name AT %QX0.0 : BOOL` in a `CONFIGURATION`'s `VAR_GLOBAL`).
//!
//! A located variable is a named variable slot, like a program-level `AT`
//! variable: the VM has no separate process image. So a located global is
//! read and written from a PROGRAM through `VAR_EXTERNAL` exactly like any
//! other global, and keeps its initial value.

// The example of docs/reference/language/variables/io-qualifiers.rst.
// start_button (0) and motor_output (1) are FALSE, scan_count (2) counts
// the scan.
e2e_i32!(
    end_to_end_when_located_globals_used_via_external_then_values_updated,
    "
CONFIGURATION config
  VAR_GLOBAL
    start_button AT %IX0.0 : BOOL;
    motor_output AT %QX0.0 : BOOL;
    scan_count AT %MW2 : INT;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    start_button : BOOL;
    motor_output : BOOL;
    scan_count : INT;
  END_VAR
  motor_output := start_button;
  scan_count := scan_count + 1;
END_PROGRAM
",
    &[(0, 0), (1, 0), (2, 1)],
);

// level (0) keeps its initial value; lamp (1) and count (2) are written from
// it, and the symbolic global (3) declared after them keeps its own slot.
e2e_i32!(
    end_to_end_when_located_global_has_initial_value_then_external_reads_it,
    "
CONFIGURATION config
  VAR_GLOBAL
    level AT %IW2 : INT := 7;
    lamp AT %QX0.1 : BOOL;
    count AT %MW4 : INT;
    plain : INT := 5;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    level : INT;
    lamp : BOOL;
    count : INT;
    plain : INT;
  END_VAR
  lamp := level > 5;
  count := level + plain;
END_PROGRAM
",
    &[(0, 7), (1, 1), (2, 12), (3, 5)],
);

// limit (0) is a located constant, read through a VAR_EXTERNAL CONSTANT.
e2e_i32!(
    end_to_end_when_located_global_constant_then_external_constant_reads_it,
    "
CONFIGURATION config
  VAR_GLOBAL CONSTANT
    limit AT %MW8 : INT := 3;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL CONSTANT
    limit : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := limit * 2;
END_PROGRAM
",
    &[(0, 3), (1, 6)],
);
