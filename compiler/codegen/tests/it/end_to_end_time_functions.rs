//! End-to-end integration tests for IEC 61131-3 Table 35 time functions.

use crate::common::{date, datetime, time, Duration};

// =============================================================================
// Group 1: Direct i32 operations (same units)
// =============================================================================

e2e!(
    add_time_when_two_durations_then_returns_sum,
    "PROGRAM main VAR x : TIME; END_VAR x := ADD_TIME(T#2s, T#3s); END_PROGRAM",
    &[("x", Duration::seconds(5))],
);

e2e!(
    sub_time_when_two_durations_then_returns_difference,
    "PROGRAM main VAR x : TIME; END_VAR x := SUB_TIME(T#5s, T#2s); END_PROGRAM",
    &[("x", Duration::seconds(3))],
);

e2e!(
    add_tod_time_when_duration_added_then_offsets_tod,
    "PROGRAM main VAR x : TIME_OF_DAY; END_VAR x := ADD_TOD_TIME(TOD#12:00:00, T#1h); END_PROGRAM",
    &[("x", time!(13:00))],
);

e2e!(
    sub_tod_time_when_duration_subtracted_then_offsets_tod,
    "PROGRAM main VAR x : TIME_OF_DAY; END_VAR x := SUB_TOD_TIME(TOD#14:00:00, T#1h); END_PROGRAM",
    &[("x", time!(13:00))],
);

e2e!(
    sub_tod_tod_when_two_tods_then_returns_duration,
    "PROGRAM main VAR x : TIME; END_VAR x := SUB_TOD_TOD(TOD#14:00:00, TOD#12:00:00); END_PROGRAM",
    &[("x", Duration::hours(2))],
);

// =============================================================================
// Group 2: ms-to-seconds conversion before add/sub
// =============================================================================

e2e!(
    add_dt_time_when_adding_duration_then_offsets_datetime,
    "PROGRAM main VAR x : DATE_AND_TIME; END_VAR x := ADD_DT_TIME(DT#2000-01-01-00:00:00, T#1h); END_PROGRAM",
    &[("x", datetime!(2000-01-01 1:00))],
);

e2e!(
    sub_dt_time_when_subtracting_duration_then_offsets_datetime,
    "PROGRAM main VAR x : DATE_AND_TIME; END_VAR x := SUB_DT_TIME(DT#2000-01-01-01:00:00, T#1h); END_PROGRAM",
    &[("x", datetime!(2000-01-01 0:00))],
);

e2e!(
    concat_date_tod_when_date_and_tod_then_returns_dt,
    "PROGRAM main VAR x : DATE_AND_TIME; END_VAR x := CONCAT_DATE_TOD(D#2000-01-01, TOD#12:00:00); END_PROGRAM",
    &[("x", datetime!(2000-01-01 12:00))],
);

// =============================================================================
// Group 5: datetime decomposition
// =============================================================================

e2e!(
    dt_to_date_when_datetime_then_returns_date,
    "PROGRAM main VAR x : DATE; END_VAR x := DT_TO_DATE(DT#2000-01-01-12:00:00); END_PROGRAM",
    &[("x", date!(2000 - 01 - 01))],
);

e2e!(
    dt_to_tod_when_datetime_then_returns_tod,
    "PROGRAM main VAR x : TIME_OF_DAY; END_VAR x := DT_TO_TOD(DT#2000-01-01-12:00:00); END_PROGRAM",
    &[("x", time!(12:00))],
);

e2e!(
    date_and_time_to_date_when_datetime_then_returns_date,
    "PROGRAM main VAR x : DATE; END_VAR x := DATE_AND_TIME_TO_DATE(DT#2000-01-01-12:00:00); END_PROGRAM",
    &[("x", date!(2000-01-01))],
);

e2e!(
    date_and_time_to_time_of_day_when_datetime_then_returns_tod,
    "PROGRAM main VAR x : TIME_OF_DAY; END_VAR x := DATE_AND_TIME_TO_TIME_OF_DAY(DT#2000-01-01-12:00:00); END_PROGRAM",
    &[("x", time!(12:00))],
);

// =============================================================================
// Group 3: seconds-to-ms conversion after sub
// =============================================================================

e2e!(
    sub_dt_dt_when_two_datetimes_then_returns_duration_ms,
    "PROGRAM main VAR x : TIME; END_VAR x := SUB_DT_DT(DT#2000-01-01-01:00:00, DT#2000-01-01-00:00:00); END_PROGRAM",
    &[("x", Duration::hours(1))],
);

e2e!(
    sub_date_date_when_two_dates_then_returns_duration_ms,
    "PROGRAM main VAR x : TIME; END_VAR x := SUB_DATE_DATE(D#2000-01-02, D#2000-01-01); END_PROGRAM",
    &[("x", Duration::days(1))],
);

// =============================================================================
// Group 4: MUL_TIME / DIV_TIME
// =============================================================================

e2e!(
    mul_time_when_integer_multiplier_then_scales,
    "PROGRAM main VAR x : TIME; END_VAR x := MUL_TIME(T#2s, 3); END_PROGRAM",
    &[("x", Duration::seconds(6))],
);

e2e!(
    mul_time_when_real_multiplier_then_scales_and_truncates,
    "PROGRAM main VAR x : TIME; END_VAR x := MUL_TIME(T#3s, REAL#1.5); END_PROGRAM",
    &[("x", Duration::milliseconds(4500))],
);

e2e!(
    div_time_when_integer_divisor_then_divides,
    "PROGRAM main VAR x : TIME; END_VAR x := DIV_TIME(T#6s, 3); END_PROGRAM",
    &[("x", Duration::seconds(2))],
);

e2e!(
    div_time_when_real_divisor_then_divides,
    "PROGRAM main VAR x : TIME; END_VAR x := DIV_TIME(T#5s, REAL#2.5); END_PROGRAM",
    &[("x", Duration::seconds(2))],
);
