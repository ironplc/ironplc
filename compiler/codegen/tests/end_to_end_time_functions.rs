//! End-to-end integration tests for IEC 61131-3 Table 35 time functions.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

// =============================================================================
// Group 1: Direct i32 operations (same units)
// =============================================================================

#[test]
fn add_time_when_two_durations_then_returns_sum() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := ADD_TIME(T#2s, T#3s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#2s = 2000ms, T#3s = 3000ms, sum = 5000ms
    assert_eq!(bufs.vars[0].as_i32(), 5000);
}

#[test]
fn sub_time_when_two_durations_then_returns_difference() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := SUB_TIME(T#5s, T#2s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#5s = 5000ms, T#2s = 2000ms, diff = 3000ms
    assert_eq!(bufs.vars[0].as_i32(), 3000);
}

#[test]
fn add_tod_time_when_duration_added_then_offsets_tod() {
    let source = "
PROGRAM main
  VAR
    x : TIME_OF_DAY;
  END_VAR
  x := ADD_TOD_TIME(TOD#12:00:00, T#1h);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // TOD#12:00:00 = 43200000ms, T#1h = 3600000ms, sum = 46800000ms (13:00:00)
    assert_eq!(bufs.vars[0].as_i32(), 46_800_000);
}

#[test]
fn sub_tod_time_when_duration_subtracted_then_offsets_tod() {
    let source = "
PROGRAM main
  VAR
    x : TIME_OF_DAY;
  END_VAR
  x := SUB_TOD_TIME(TOD#14:00:00, T#1h);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // TOD#14:00:00 = 50400000ms, T#1h = 3600000ms, diff = 46800000ms (13:00:00)
    assert_eq!(bufs.vars[0].as_i32(), 46_800_000);
}

#[test]
fn sub_tod_tod_when_two_tods_then_returns_duration() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := SUB_TOD_TOD(TOD#14:00:00, TOD#12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Diff = 2h = 7200000ms
    assert_eq!(bufs.vars[0].as_i32(), 7_200_000);
}

// =============================================================================
// Group 2: ms-to-seconds conversion before add/sub
// =============================================================================

#[test]
fn add_dt_time_when_adding_duration_then_offsets_datetime() {
    let source = "
PROGRAM main
  VAR
    x : DATE_AND_TIME;
  END_VAR
  x := ADD_DT_TIME(DT#2000-01-01-00:00:00, T#1h);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // DT#2000-01-01-00:00:00 = 946684800s, T#1h = 3600000ms = 3600s
    // Result = 946684800 + 3600 = 946688400
    assert_eq!(bufs.vars[0].as_i32(), 946_688_400);
}

#[test]
fn sub_dt_time_when_subtracting_duration_then_offsets_datetime() {
    let source = "
PROGRAM main
  VAR
    x : DATE_AND_TIME;
  END_VAR
  x := SUB_DT_TIME(DT#2000-01-01-01:00:00, T#1h);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // DT#2000-01-01-01:00:00 = 946688400s, T#1h = 3600s
    // Result = 946688400 - 3600 = 946684800
    assert_eq!(bufs.vars[0].as_i32(), 946_684_800);
}

#[test]
fn concat_date_tod_when_date_and_tod_then_returns_dt() {
    let source = "
PROGRAM main
  VAR
    x : DATE_AND_TIME;
  END_VAR
  x := CONCAT_DATE_TOD(D#2000-01-01, TOD#12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // D#2000-01-01 = 946684800s, TOD#12:00:00 = 43200000ms = 43200s
    // Result = 946684800 + 43200 = 946728000
    assert_eq!(bufs.vars[0].as_i32(), 946_728_000);
}

// =============================================================================
// Group 5: datetime decomposition
// =============================================================================

#[test]
fn dt_to_date_when_datetime_then_returns_date() {
    let source = "
PROGRAM main
  VAR
    x : DATE;
  END_VAR
  x := DT_TO_DATE(DT#2000-01-01-12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // DT#2000-01-01-12:00:00 = 946728000s
    // DATE = 946728000 - (946728000 % 86400) = 946728000 - 43200 = 946684800
    assert_eq!(bufs.vars[0].as_i32(), 946_684_800);
}

#[test]
fn dt_to_tod_when_datetime_then_returns_tod() {
    let source = "
PROGRAM main
  VAR
    x : TIME_OF_DAY;
  END_VAR
  x := DT_TO_TOD(DT#2000-01-01-12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // DT#2000-01-01-12:00:00 = 946728000s
    // TOD = (946728000 % 86400) * 1000 = 43200 * 1000 = 43200000ms
    assert_eq!(bufs.vars[0].as_i32(), 43_200_000);
}

#[test]
fn date_and_time_to_date_when_datetime_then_returns_date() {
    let source = "
PROGRAM main
  VAR
    x : DATE;
  END_VAR
  x := DATE_AND_TIME_TO_DATE(DT#2000-01-01-12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 946_684_800);
}

#[test]
fn date_and_time_to_time_of_day_when_datetime_then_returns_tod() {
    let source = "
PROGRAM main
  VAR
    x : TIME_OF_DAY;
  END_VAR
  x := DATE_AND_TIME_TO_TIME_OF_DAY(DT#2000-01-01-12:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 43_200_000);
}

// =============================================================================
// Group 3: seconds-to-ms conversion after sub
// =============================================================================

#[test]
fn sub_dt_dt_when_two_datetimes_then_returns_duration_ms() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := SUB_DT_DT(DT#2000-01-01-01:00:00, DT#2000-01-01-00:00:00);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Diff = 3600s * 1000 = 3600000ms
    assert_eq!(bufs.vars[0].as_i32(), 3_600_000);
}

#[test]
fn sub_date_date_when_two_dates_then_returns_duration_ms() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := SUB_DATE_DATE(D#2000-01-02, D#2000-01-01);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Diff = 86400s * 1000 = 86400000ms
    assert_eq!(bufs.vars[0].as_i32(), 86_400_000);
}

// =============================================================================
// Group 4: MUL_TIME / DIV_TIME
// =============================================================================

#[test]
fn mul_time_when_integer_multiplier_then_scales() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := MUL_TIME(T#2s, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#2s = 2000ms, 2000 * 3 = 6000ms
    assert_eq!(bufs.vars[0].as_i32(), 6000);
}

#[test]
fn mul_time_when_real_multiplier_then_scales_and_truncates() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := MUL_TIME(T#3s, REAL#1.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#3s = 3000ms, 3000 * 1.5 = 4500ms
    assert_eq!(bufs.vars[0].as_i32(), 4500);
}

#[test]
fn div_time_when_integer_divisor_then_divides() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := DIV_TIME(T#6s, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#6s = 6000ms, 6000 / 3 = 2000ms
    assert_eq!(bufs.vars[0].as_i32(), 2000);
}

#[test]
fn div_time_when_real_divisor_then_divides() {
    let source = "
PROGRAM main
  VAR
    x : TIME;
  END_VAR
  x := DIV_TIME(T#5s, REAL#2.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // T#5s = 5000ms, 5000 / 2.5 = 2000ms
    assert_eq!(bufs.vars[0].as_i32(), 2000);
}
