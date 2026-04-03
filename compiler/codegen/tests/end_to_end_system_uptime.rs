//! End-to-end tests for system uptime global variables.
//!
//! These tests verify that `__SYSTEM_UP_TIME` (TIME, i32 ms) and
//! `__SYSTEM_UP_LTIME` (LTIME, i64 ms) are injected by the compiler
//! and written by the VM before each scan round.

mod common;
use common::{parse_and_compile, parse_and_run_rounds};
use ironplc_container::{VarIndex, FLAG_HAS_SYSTEM_UPTIME};
use ironplc_parser::options::{CompilerOptions, Dialect};

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
fn vm_when_uptime_enabled_then_globals_shift_by_two() {
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
    parse_and_run_rounds(source, &rusty_options(), |vm| {
        vm.run_round(5_000_000).unwrap();

        // Index 0: __SYSTEM_UP_TIME (i32 ms)
        // Index 1: __SYSTEM_UP_LTIME (i64 ms)
        // Index 2: user_var (initial value 42)
        // Index 3: result (should be 42)
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 5000);
        assert_eq!(vm.read_variable_i64(VarIndex::new(1)).unwrap(), 5000);
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 42);
        assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 42);
    });
}

#[test]
fn vm_when_two_rounds_then_uptime_updates() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR
    t : TIME;
  END_VAR
  VAR_EXTERNAL
    __SYSTEM_UP_TIME : TIME;
  END_VAR
  t := __SYSTEM_UP_TIME;
END_PROGRAM
";
    parse_and_run_rounds(source, &rusty_options(), |vm| {
        // Round 1: 1 second
        vm.run_round(1_000_000).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 1000);
        assert_eq!(vm.read_variable_i64(VarIndex::new(1)).unwrap(), 1000);

        // Round 2: 5 seconds
        vm.run_round(5_000_000).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 5000);
        assert_eq!(vm.read_variable_i64(VarIndex::new(1)).unwrap(), 5000);
    });
}

#[test]
fn vm_when_uptime_exceeds_i32_max_then_time_wraps_but_ltime_does_not() {
    let source = "
PROGRAM main
VAR
    x : INT;
END_VAR
    x := 1;
END_PROGRAM
";
    parse_and_run_rounds(source, &rusty_options(), |vm| {
        // ~25 days in microseconds (exceeds i32 max ms of ~24.8 days)
        let time_us: u64 = 25 * 24 * 3600 * 1_000_000;
        let time_ms = (time_us / 1000) as i64;
        vm.run_round(time_us).unwrap();

        // i32 should have wrapped
        let up_time = vm.read_variable(VarIndex::new(0)).unwrap();
        assert_eq!(up_time, time_ms as i32);

        // i64 should be exact
        let up_ltime = vm.read_variable_i64(VarIndex::new(1)).unwrap();
        assert_eq!(up_ltime, time_ms);

        // Verify they differ (i32 wrapped)
        assert_ne!(up_time as i64, up_ltime);
    });
}
