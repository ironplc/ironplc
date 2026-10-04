//! End-to-end tests for system uptime global variables.
//!
//! These tests verify that `__SYSTEM_UP_TIME` (TIME) and
//! `__SYSTEM_UP_LTIME` (LTIME) are injected by the compiler and written by
//! the VM before each scan round. `compile_system_uptime.rs` checks how the
//! compiler lays them out.

use crate::common::{run_scans, Duration};
use ironplc_parser::options::{CompilerOptions, Dialect};

fn rusty_options() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Rusty)
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
    run_scans(source, &rusty_options(), |session| {
        // Round 1: 1 second
        session.scan(1_000_000).unwrap();
        assert_eq!(session.read("__SYSTEM_UP_TIME"), Duration::seconds(1));
        assert_eq!(session.read("__SYSTEM_UP_LTIME"), Duration::seconds(1));

        // Round 2: 5 seconds
        session.scan(5_000_000).unwrap();
        assert_eq!(session.read("__SYSTEM_UP_TIME"), Duration::seconds(5));
        assert_eq!(session.read("__SYSTEM_UP_LTIME"), Duration::seconds(5));
    });
}

#[test]
fn vm_when_direct_access_without_var_external_then_uptime_reads_correctly() {
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
  t := __SYSTEM_UP_TIME;
END_PROGRAM
";
    run_scans(source, &rusty_options(), |session| {
        session.scan(3_000_000).unwrap();
        assert_eq!(session.read("__SYSTEM_UP_TIME"), Duration::seconds(3));
        assert_eq!(session.read("t"), Duration::seconds(3));
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
    run_scans(source, &rusty_options(), |session| {
        // ~25 days, past the ~24.8 days a TIME holds (32-bit milliseconds,
        // ADR-0021), so TIME wraps while LTIME holds the uptime exactly.
        let uptime = Duration::days(25);
        session.scan(uptime.whole_microseconds() as u64).unwrap();

        let wrapped = Duration::milliseconds(i64::from(uptime.whole_milliseconds() as i32));
        assert_ne!(wrapped, uptime);
        assert_eq!(session.read("__SYSTEM_UP_TIME"), wrapped);
        assert_eq!(session.read("__SYSTEM_UP_LTIME"), uptime);
    });
}
