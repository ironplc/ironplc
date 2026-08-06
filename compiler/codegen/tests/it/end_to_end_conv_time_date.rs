//! End-to-end tests for time/date type conversions.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// Time/date reinterpret conversions with an exact expected result. TIME is
/// milliseconds, TOD is milliseconds since midnight. These are reinterpret
/// casts, so they stay a parametrized table rather than a property test.
#[rstest]
#[case::time_to_dword("t : TIME := T#5s", "DWORD", "TIME_TO_DWORD(t)", 5000)]
#[case::dword_to_time("dw : DWORD := 3000", "TIME", "DWORD_TO_TIME(dw)", 3000)]
#[case::time_to_dint("t : TIME := T#5s", "DINT", "TIME_TO_DINT(t)", 5000)]
#[case::time_to_int("t : TIME := T#2s", "INT", "TIME_TO_INT(t)", 2000)]
// 12:30:00 = 12*3600*1000 + 30*60*1000 = 45_000_000 ms since midnight
#[case::tod_to_dword("t : TOD := TOD#12:30:00", "DWORD", "TOD_TO_DWORD(t)", 45_000_000)]
fn time_date_exact(
    #[case] src_decl: &str,
    #[case] result_type: &str,
    #[case] call: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    {src_decl};
    result : {result_type};
  END_VAR
  result := {call};
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u32, expected as u32);
}

/// Date/datetime conversions stored as seconds since 1970-01-01; only their
/// non-zero-ness is asserted (the absolute epoch value is not pinned).
#[rstest]
#[case::date_to_dword("d : DATE := D#2024-06-15", "DWORD", "DATE_TO_DWORD(d)")]
#[case::date_to_udint("d : DATE := D#2024-06-15", "UDINT", "DATE_TO_UDINT(d)")]
#[case::dt_to_dword("d : DT := DT#2024-06-15-12:30:00", "DWORD", "DT_TO_DWORD(d)")]
fn time_date_nonzero(#[case] src_decl: &str, #[case] result_type: &str, #[case] call: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    {src_decl};
    result : {result_type};
  END_VAR
  result := {call};
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert!(bufs.vars[1].as_i32() as u32 > 0);
}

e2e_f32_near!(
    end_to_end_when_time_to_real_then_correct,
    1e-1,
    "
PROGRAM main
  VAR
    t : TIME := T#5s;
    result : REAL;
  END_VAR
  result := TIME_TO_REAL(t);
END_PROGRAM
",
    &[(1, 5000.0)],
);
