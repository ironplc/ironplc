//! End-to-end integration tests for located global variables
//! (`name AT %QX0.0 : BOOL` in a `VAR_GLOBAL` of a `CONFIGURATION` or a
//! `RESOURCE`).
//!
//! The bytecode VM has no process image, so a located global is a variable
//! like any other, as a program's located variable is: a program reads and
//! writes it through `VAR_EXTERNAL`, and it starts at its initial value. A
//! located global with no name is no variable at all.

// The example of docs/reference/language/variables/io-qualifiers.rst.
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
    &[("start_button", 0), ("motor_output", 0), ("scan_count", 1)],
);

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
    &[("level", 7), ("lamp", 1), ("count", 12), ("plain", 5)],
);

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
    &[("limit", 3), ("result", 6)],
);

e2e_i32!(
    end_to_end_when_resource_global_located_then_external_writes_it,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count AT %MW4 : INT := 3;
    END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    count : INT;
  END_VAR
  count := count + 1;
END_PROGRAM
",
    &[("count", 4)],
);

e2e_i32!(
    end_to_end_when_located_global_without_name_then_other_globals_unaffected,
    "
CONFIGURATION config
  VAR_GLOBAL
    AT %MW2 : INT := 9;
    plain : INT := 5;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    plain : INT;
  END_VAR
  plain := plain + 1;
END_PROGRAM
",
    &[("plain", 6)],
);
