//! Compile tests for the system uptime globals' header flag and layout.
//!
//! A container with `FLAG_HAS_SYSTEM_UPTIME` set promises the VM that
//! `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` are its first two slots, because
//! the VM writes the uptime there before each scan. That layout is the
//! subject here; `end_to_end_system_uptime.rs` reads the uptime by name.

use ironplc_container::{VarIndex, FLAG_HAS_SYSTEM_UPTIME};
use ironplc_parser::options::{CompilerOptions, Dialect};

use crate::common::{parse_and_compile, vm_var_index};

fn rusty_options() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Rusty)
}

#[test]
fn compile_when_uptime_enabled_then_header_flag_set() {
    let source = "
PROGRAM main
VAR
    x : INT;
END_VAR
    x := 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &rusty_options());
    assert_ne!(container.header.flags & FLAG_HAS_SYSTEM_UPTIME, 0);
}

#[test]
fn compile_when_uptime_disabled_then_header_flag_clear() {
    let source = "
PROGRAM main
VAR
    x : INT;
END_VAR
    x := 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    assert_eq!(container.header.flags & FLAG_HAS_SYSTEM_UPTIME, 0);
}

#[test]
fn compile_when_uptime_enabled_then_uptime_globals_take_first_two_slots() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL
    user_var : INT := 42;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    user_var : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := user_var;
END_PROGRAM
";
    let container = parse_and_compile(source, &rusty_options());

    let slot = |name| vm_var_index(&container, name);
    assert_eq!(slot("__SYSTEM_UP_TIME"), VarIndex::new(0));
    assert_eq!(slot("__SYSTEM_UP_LTIME"), VarIndex::new(1));
    assert_eq!(slot("user_var"), VarIndex::new(2));
    assert_eq!(slot("result"), VarIndex::new(3));
}
