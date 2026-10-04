//! Conformance tests for how the end-to-end harness observes a program
//! (`specs/design/end-to-end-test-observation.md`, REQ-OBS-codegen-*): by
//! variable name and IEC value, never by slot or storage encoding.

use ironplc_parser::options::{CompilerOptions, Dialect};

use crate::common::{
    assert_run, assert_run_with, date, datetime, drive_fb, expect, pulse, run, run_scans, time,
    write, Duration, Snapshot, Value,
};

fn snapshot(source: &str) -> Snapshot {
    Snapshot::run(source, &CompilerOptions::default())
}

fn snapshot_ed3(source: &str) -> Snapshot {
    Snapshot::run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
}

// REQ-OBS-codegen-001
#[test]
fn observe_when_var_block_reversed_then_names_read_the_same_values() {
    let forward = snapshot(
        "PROGRAM main VAR a : DINT; b : BOOL; c : REAL; END_VAR a := 1; b := TRUE; c := 2.5; END_PROGRAM",
    );
    let reversed = snapshot(
        "PROGRAM main VAR c : REAL; b : BOOL; a : DINT; END_VAR a := 1; b := TRUE; c := 2.5; END_PROGRAM",
    );
    for name in ["a", "b", "c"] {
        assert_eq!(forward.read(name), reversed.read(name), "`{name}`");
    }
}

// REQ-OBS-codegen-002
#[test]
fn observe_when_name_differs_in_case_then_reads_same_variable() {
    let snapshot = snapshot("PROGRAM main VAR Count : DINT; END_VAR Count := 7; END_PROGRAM");
    assert_eq!(snapshot.read("COUNT"), 7);
    assert_eq!(snapshot.read("count"), 7);
}

// REQ-OBS-codegen-003
#[test]
fn observe_when_global_declared_external_then_reads_by_bare_name() {
    let snapshot = snapshot(
        "
CONFIGURATION config
  VAR_GLOBAL
    shared : INT := 42;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    shared : INT;
  END_VAR
  shared := shared + 1;
END_PROGRAM
",
    );
    assert_eq!(snapshot.read("shared"), 43);
}

// REQ-OBS-codegen-004
#[test]
fn observe_when_uptime_globals_enabled_then_user_names_read_the_same_values() {
    let source = "PROGRAM main VAR a : DINT; b : DINT; END_VAR a := 1; b := 2; END_PROGRAM";
    let without = snapshot(source);
    let with = Snapshot::run(
        source,
        &CompilerOptions {
            allow_system_uptime_global: true,
            ..CompilerOptions::default()
        },
    );
    for name in ["a", "b"] {
        assert_eq!(without.read(name), with.read(name), "`{name}`");
    }
}

// REQ-OBS-codegen-005
#[test]
fn observe_when_function_declares_same_name_then_reads_program_variable() {
    let snapshot = snapshot(
        "
FUNCTION scaled : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR x : DINT; END_VAR
  x := a * 100;
  scaled := x;
END_FUNCTION

PROGRAM main
  VAR x : DINT; y : DINT; END_VAR
  x := 7;
  y := scaled(1);
END_PROGRAM
",
    );
    assert_eq!(snapshot.read("x"), 7);
    assert_eq!(snapshot.read("y"), 100);
}

// REQ-OBS-codegen-006
#[test]
#[should_panic(expected = "declares no variable `missing`; it declares present")]
fn observe_when_name_not_declared_then_fails_listing_declared_names() {
    snapshot("PROGRAM main VAR present : DINT; END_VAR present := 1; END_PROGRAM").read("missing");
}

// REQ-OBS-codegen-020
#[test]
fn observe_when_bool_then_reads_bool() {
    let snapshot = snapshot("PROGRAM main VAR b : BOOL; END_VAR b := TRUE; END_PROGRAM");
    assert_eq!(snapshot.read("b"), Value::Bool(true));
}

// REQ-OBS-codegen-021
#[test]
fn observe_when_signed_integer_then_reads_its_value() {
    let snapshot = snapshot("PROGRAM main VAR s : SINT; END_VAR s := -1; END_PROGRAM");
    assert_eq!(snapshot.read("s"), Value::Int(-1));
}

// REQ-OBS-codegen-022
#[test]
fn observe_when_unsigned_all_ones_then_reads_without_sign_extension() {
    let snapshot = snapshot(
        "PROGRAM main VAR u : UDINT; w : LWORD; END_VAR u := 16#FFFF_FFFF; w := 16#FFFF_FFFF_FFFF_FFFF; END_PROGRAM",
    );
    assert_eq!(snapshot.read("u"), Value::Int(4_294_967_295));
    assert_eq!(snapshot.read("w"), Value::Int(18_446_744_073_709_551_615));
}

// REQ-OBS-codegen-023
#[test]
fn observe_when_real_then_reads_f32_widened_and_lreal_unchanged() {
    let snapshot =
        snapshot("PROGRAM main VAR r : REAL; l : LREAL; END_VAR r := 0.1; l := 0.1; END_PROGRAM");
    assert_eq!(snapshot.read("r"), Value::Real(f64::from(0.1_f32)));
    assert_eq!(snapshot.read("l"), Value::Real(0.1));
}

// REQ-OBS-codegen-024
#[test]
fn observe_when_enumeration_and_subrange_then_read_ordinal_and_value() {
    let snapshot = snapshot(
        "
TYPE
  Color : (RED, GREEN, BLUE);
  Level : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    c : Color;
    level : Level;
  END_VAR
  c := GREEN;
  level := 42;
END_PROGRAM
",
    );
    assert_eq!(snapshot.read("c"), Value::Int(1));
    assert_eq!(snapshot.read("level"), Value::Int(42));
}

// REQ-OBS-codegen-025
#[test]
fn observe_when_string_then_reads_current_content() {
    let snapshot = snapshot(
        "PROGRAM main VAR s : STRING[10]; w : WSTRING[10]; END_VAR s := 'hi'; w := \"wide\"; END_PROGRAM",
    );
    assert_eq!(snapshot.read("s"), "hi");
    assert_eq!(snapshot.read("w"), "wide");
}

// REQ-OBS-codegen-030
#[test]
fn observe_when_time_and_ltime_then_read_durations() {
    let snapshot = snapshot_ed3(
        "PROGRAM main VAR t : TIME; lt : LTIME; END_VAR t := T#1.5s; lt := LTIME#2d; END_PROGRAM",
    );
    assert_eq!(snapshot.read("t"), Duration::seconds_f64(1.5));
    assert_eq!(snapshot.read("lt"), Duration::days(2));
}

// REQ-OBS-codegen-031
#[test]
fn observe_when_duration_negative_then_reads_negative_duration() {
    let snapshot = snapshot("PROGRAM main VAR t : TIME; END_VAR t := T#-5s; END_PROGRAM");
    assert_eq!(snapshot.read("t"), -Duration::seconds(5));
}

// REQ-OBS-codegen-032
#[test]
fn observe_when_date_then_reads_calendar_date() {
    let snapshot = snapshot_ed3(
        "PROGRAM main VAR d : DATE; ld : LDATE; END_VAR d := D#2024-01-01; ld := LDATE#2024-01-01; END_PROGRAM",
    );
    assert_eq!(snapshot.read("d"), date!(2024 - 01 - 01));
    assert_eq!(snapshot.read("ld"), date!(2024 - 01 - 01));
}

// REQ-OBS-codegen-033
#[test]
fn observe_when_time_of_day_then_reads_time_of_day() {
    let snapshot =
        snapshot("PROGRAM main VAR t : TIME_OF_DAY; END_VAR t := TOD#12:30:15.5; END_PROGRAM");
    assert_eq!(snapshot.read("t"), time!(12:30:15.5));
}

// REQ-OBS-codegen-034
#[test]
fn observe_when_date_and_time_then_reads_date_and_time() {
    let snapshot = snapshot(
        "PROGRAM main VAR stamp : DATE_AND_TIME; END_VAR stamp := DT#2024-01-01-12:30:00; END_PROGRAM",
    );
    assert_eq!(snapshot.read("stamp"), datetime!(2024-01-01 12:30));
}

// REQ-OBS-codegen-035
#[test]
#[should_panic(expected = "is not an integer")]
fn observe_when_integer_expected_from_time_then_fails() {
    assert_run::<i32>(
        "PROGRAM main VAR t : TIME; END_VAR t := T#1.5s; END_PROGRAM",
        &[("t", 1500)],
    );
}

// REQ-OBS-codegen-035
#[test]
#[should_panic(expected = "its type cannot hold")]
fn observe_when_integer_written_to_date_then_fails() {
    run_scans(
        "PROGRAM main VAR d : DATE; END_VAR END_PROGRAM",
        &CompilerOptions::default(),
        |session| session.write("d", 1_704_067_200),
    );
}

// REQ-OBS-codegen-040
#[test]
fn observe_when_value_written_between_scans_then_next_scan_reads_it() {
    run_scans(
        "PROGRAM main VAR input : DINT; output : DINT; END_VAR output := input * 2; END_PROGRAM",
        &CompilerOptions::default(),
        |session| {
            session.write("input", 21);
            session.scan(0).unwrap();
            assert_eq!(session.read("output"), 42);
        },
    );
}

// REQ-OBS-codegen-041
#[test]
#[should_panic(expected = "300 is out of range")]
fn observe_when_written_value_does_not_fit_declared_type_then_fails() {
    run_scans(
        "PROGRAM main VAR small : USINT; END_VAR END_PROGRAM",
        &CompilerOptions::default(),
        |session| session.write("small", 300),
    );
}

// REQ-OBS-codegen-042
#[test]
fn observe_when_program_counts_scans_then_value_persists_across_scans() {
    run_scans(
        "PROGRAM main VAR count : DINT; END_VAR count := count + 1; END_PROGRAM",
        &CompilerOptions::default(),
        |session| {
            for _ in 0..5 {
                session.scan(0).unwrap();
            }
            assert_eq!(session.read("count"), 5);
        },
    );
}

// REQ-OBS-codegen-043
e2e_i32!(
    observe_when_macro_asserts_then_takes_name_and_value,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 10; y := x + 32; END_PROGRAM",
    &[("x", 10), ("y", 42)],
);

// REQ-OBS-codegen-044
#[test]
#[should_panic(expected = "4294967295 does not fit i32")]
fn observe_when_i32_expected_from_udint_beyond_i32_then_fails() {
    assert_run::<i32>(
        "PROGRAM main VAR u : UDINT; END_VAR u := 16#FFFF_FFFF; END_PROGRAM",
        &[("u", -1)],
    );
}

// REQ-OBS-codegen-044
#[test]
#[should_panic(expected = "an f32 is compared against a REAL, not an LREAL")]
fn observe_when_f32_expected_from_lreal_then_fails() {
    assert_run::<f32>(
        "PROGRAM main VAR l : LREAL; END_VAR l := 1.5; END_PROGRAM",
        &[("l", 1.5)],
    );
}

// REQ-OBS-codegen-045
#[test]
fn observe_when_bool_expected_as_integer_then_true_is_one_and_false_zero() {
    assert_run_with::<i32>(
        "PROGRAM main VAR t : BOOL; f : BOOL; END_VAR t := TRUE; f := FALSE; END_PROGRAM",
        &CompilerOptions::default(),
        &[("t", 1), ("f", 0)],
    );
}

// REQ-OBS-codegen-045
#[test]
fn observe_when_one_and_zero_written_to_bool_then_true_and_false() {
    run_scans(
        "PROGRAM main VAR b : BOOL; copy : BOOL; END_VAR copy := b; END_PROGRAM",
        &CompilerOptions::default(),
        |session| {
            session.write("b", 1);
            session.scan(0).unwrap();
            assert_eq!(session.read("copy"), Value::Bool(true));
            session.write("b", 0);
            session.scan(1).unwrap();
            assert_eq!(session.read("copy"), Value::Bool(false));
        },
    );
}

// REQ-OBS-codegen-046
#[test]
fn observe_when_function_block_steps_name_variables_then_drive_by_name() {
    drive_fb(
        "
PROGRAM main
  VAR counter : CTU; up : BOOL; reset : BOOL; done : BOOL; count : INT; END_VAR
  counter(CU := up, R := reset, PV := 3, Q => done, CV => count);
END_PROGRAM
",
        &CompilerOptions::default(),
        &[
            pulse("up", 3, 0),
            expect("count", 3),
            expect("done", 1),
            write("reset", 1),
            run(10),
            expect("count", 0),
            expect("done", 0),
        ],
    );
}

// REQ-OBS-codegen-047
#[test]
fn observe_when_duration_written_and_expected_then_compared_as_durations() {
    run_scans(
        "PROGRAM main VAR pt : TIME; doubled : TIME; END_VAR doubled := pt + pt; END_PROGRAM",
        &CompilerOptions::default(),
        |session| {
            session.write("pt", Duration::seconds(2));
            session.scan(0).unwrap();
            assert_eq!(session.read_as::<Duration>("doubled"), Duration::seconds(4));
        },
    );
    drive_fb(
        "
PROGRAM main
  VAR timer : TON; preset : TIME; elapsed : TIME; done : BOOL; END_VAR
  timer(IN := TRUE, PT := preset, Q => done, ET => elapsed);
END_PROGRAM
",
        &CompilerOptions::default(),
        &[
            write("preset", Duration::seconds(2)),
            run(0),
            run(1_000_000),
            expect("elapsed", Duration::seconds(1)),
            expect("done", 0),
            run(3_000_000),
            expect("elapsed", Duration::seconds(2)),
            expect("done", 1),
        ],
    );
}
