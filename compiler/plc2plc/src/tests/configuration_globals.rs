//! `VAR_GLOBAL` blocks inside `CONFIGURATION` and `RESOURCE`.
//!
//! The grammar allows at most one `VAR_GLOBAL` block in each, and only at a
//! fixed position: before the resources of a configuration, and before the
//! tasks and programs of a resource (IEC 61131-3 Edition 2, B.1.7).

use super::common::*;

#[test]
fn write_to_string_when_resource_has_global_then_round_trips() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      counter : INT;
    END_VAR
    TASK plc_task (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_resource_has_several_globals_then_round_trips() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      a : INT;
      b : BOOL := TRUE;
    END_VAR
    PROGRAM plc_task_instance : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_resource_global_block_retain_then_writes_one_block() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL RETAIN
      a : INT;
      b : INT;
    END_VAR
    PROGRAM plc_task_instance : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());
    // The parser copies the block qualifier onto each declaration, so the
    // AST cannot tell one block from one block per declaration.
    assert_eq!(rendered.matches("VAR_GLOBAL").count(), 1);
}

#[test]
fn write_to_string_when_configuration_and_resource_have_globals_then_round_trips() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    shared : INT;
    flag : BOOL;
  END_VAR
  RESOURCE resource1 ON PLC
    VAR_GLOBAL CONSTANT
      limit : INT := 10;
    END_VAR
    TASK plc_task (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : plc_prg;
  END_RESOURCE
  VAR_CONFIG
    resource1.plc_task_instance.x AT %QB1 : BYTE;
  END_VAR
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_configuration_has_located_globals_then_round_trips() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    level AT %IW2 : INT := 7;
    AT %MW4 : INT;
    plain : BOOL;
  END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM plc_task_instance : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_resource_global_block_retain_located_then_round_trips() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL RETAIN
      lamp AT %QX0.1 : BOOL;
    END_VAR
    PROGRAM plc_task_instance : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}
