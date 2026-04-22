//! End-to-end tests for time/date type conversions.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// TIME ↔ integer conversions: T#5s = 5000ms; T#2s = 2000ms. vars[1] is `result`.
#[rstest]
#[case::time_to_dword(
    "t : TIME := T#5s; result : DWORD;",
    "TIME_TO_DWORD(t)",
    5000
)]
#[case::dword_to_time(
    "dw : DWORD := 3000; result : TIME;",
    "DWORD_TO_TIME(dw)",
    3000
)]
#[case::time_to_dint(
    "t : TIME := T#5s; result : DINT;",
    "TIME_TO_DINT(t)",
    5000
)]
#[case::time_to_int(
    "t : TIME := T#2s; result : INT;",
    "TIME_TO_INT(t)",
    2000
)]
// TOD is stored as unsigned milliseconds since midnight; 12:30:00 = 45_000_000.
#[case::tod_to_dword(
    "t : TOD := TOD#12:30:00; result : DWORD;",
    "TOD_TO_DWORD(t)",
    45_000_000
)]
fn end_to_end_time_date_exact(
    #[case] vars: &str,
    #[case] call: &str,
    #[case] expected: i32,
) {
    let source = format!("PROGRAM main VAR {vars} END_VAR result := {call}; END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// TIME_TO_REAL: f32 result close to 5000.0.
e2e_f32_near!(
    end_to_end_when_time_to_real_then_correct,
    1e-1,
    "PROGRAM main VAR t : TIME := T#5s; result : REAL; END_VAR result := TIME_TO_REAL(t); END_PROGRAM",
    &[(1, 5000.0)],
);

// DATE / DT conversions: exact value is platform-specific (based on epoch in
// 1970); just confirm it's non-zero.
#[rstest]
#[case::date_to_dword(
    "d : DATE := D#2024-06-15; result : DWORD;",
    "DATE_TO_DWORD(d)"
)]
#[case::date_to_udint(
    "d : DATE := D#2024-06-15; result : UDINT;",
    "DATE_TO_UDINT(d)"
)]
#[case::dt_to_dword(
    "d : DT := DT#2024-06-15-12:30:00; result : DWORD;",
    "DT_TO_DWORD(d)"
)]
fn end_to_end_time_date_nonzero(#[case] vars: &str, #[case] call: &str) {
    let source = format!("PROGRAM main VAR {vars} END_VAR result := {call}; END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert!(bufs.vars[1].as_i32() as u32 > 0);
}
