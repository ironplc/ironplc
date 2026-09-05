//! Rendering of the IEC 61131-3 duration and calendar types.
//!
//! Every value here arrives as a raw variable-table slot, so this module owns
//! the two facts a renderer needs about each type: the unit the VM stores it in
//! and the literal syntax it displays as. Both are fixed by codegen — see
//! [ADR-0025](../../../../specs/adrs/0025-datetime-unsigned-representation.md)
//! and its amendment:
//!
//! | Type | Slot | Unit |
//! |------|------|------|
//! | `TIME` | i32 | milliseconds |
//! | `LTIME` | i64 | milliseconds |
//! | `DATE`, `DATE_AND_TIME` | u32 | seconds since 1970-01-01 |
//! | `LDATE`, `LDT` | u64 | seconds since 1970-01-01 |
//! | `TIME_OF_DAY` | u32 | milliseconds since midnight |
//! | `LTOD` | u64 | milliseconds since midnight |
//!
//! Durations render in milliseconds rather than in the largest unit that fits
//! (`T#1500ms`, not `T#1.5s`): the value is then exact, unit-stable across the
//! whole range, and reparses as the same literal. Note that IEC puts the sign
//! after the `#` — `T#-250ms`, never `-T#250ms`.

use std::format;
use std::string::String;

/// Seconds in one day.
const SECS_PER_DAY: u64 = 86_400;
/// Milliseconds in one day.
const MS_PER_DAY: u64 = 86_400_000;
/// Julian day number of 1970-01-01, the epoch all calendar types count from.
const UNIX_EPOCH_JULIAN_DAY: i64 = 2_440_588;

/// Renders a duration held as milliseconds: `T#-250ms`, `LTIME#10000ms`.
pub(super) fn format_duration(prefix: &str, ms: i64) -> String {
    format!("{prefix}#{ms}ms")
}

/// Renders a date held as seconds since 1970-01-01: `D#2024-01-15`.
///
/// Only the whole-day part is used; a sub-day remainder (which a well-formed
/// `DATE` never carries) is discarded rather than rounded.
pub(super) fn format_date(prefix: &str, secs: u64) -> String {
    let (year, month, day) = civil_from_days((secs / SECS_PER_DAY) as i64);
    format!("{prefix}#{year}-{month:02}-{day:02}")
}

/// Renders a time of day held as milliseconds since midnight:
/// `TOD#14:30:00`, or `TOD#23:59:59.999` when there is a millisecond part.
///
/// A value past midnight wraps rather than reporting an hour above 23, so the
/// rendering is always a readable clock time.
pub(super) fn format_time_of_day(prefix: &str, ms: u64) -> String {
    let ms = ms % MS_PER_DAY;
    let (h, m, s, frac) = (
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1_000,
        ms % 1_000,
    );
    if frac == 0 {
        format!("{prefix}#{h:02}:{m:02}:{s:02}")
    } else {
        format!("{prefix}#{h:02}:{m:02}:{s:02}.{frac:03}")
    }
}

/// Renders a date and time held as seconds since 1970-01-01:
/// `DT#2024-01-15-14:30:00`.
pub(super) fn format_date_and_time(prefix: &str, secs: u64) -> String {
    let (year, month, day) = civil_from_days((secs / SECS_PER_DAY) as i64);
    let within_day = secs % SECS_PER_DAY;
    let (h, m, s) = (
        within_day / 3_600,
        (within_day % 3_600) / 60,
        within_day % 60,
    );
    format!("{prefix}#{year}-{month:02}-{day:02}-{h:02}:{m:02}:{s:02}")
}

/// Converts a day count since 1970-01-01 into `(year, month, day)`.
///
/// Uses Richards' inverse-Julian-day algorithm (Meeus, *Astronomical
/// Algorithms*) so the container crate stays free of a calendar dependency —
/// it is `no_std` for its embedded consumers, and this is the only date
/// arithmetic it needs.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let j = UNIX_EPOCH_JULIAN_DAY + days;
    let f = j + 1401 + ((4 * j + 274_277) / 146_097) * 3 / 4 - 38;
    let e = 4 * f + 3;
    let g = (e % 1461) / 4;
    let h = 5 * g + 2;
    let day = (h % 153) / 5 + 1;
    let month = (h / 153 + 2) % 12 + 1;
    let year = e / 1461 - 4716 + (12 + 2 - month) / 12;
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0, "T#0ms")]
    #[case(250, "T#250ms")]
    #[case(1500, "T#1500ms")]
    #[case(-250, "T#-250ms")]
    fn format_duration_when_milliseconds_then_sign_follows_the_hash(
        #[case] ms: i64,
        #[case] expected: &str,
    ) {
        assert_eq!(format_duration("T", ms), expected);
    }

    #[test]
    fn format_duration_when_ltime_then_uses_its_own_prefix() {
        assert_eq!(format_duration("LTIME", 10_000), "LTIME#10000ms");
    }

    #[rstest]
    #[case(0, "D#1970-01-01")]
    #[case(1_705_276_800, "D#2024-01-15")]
    #[case(951_782_400, "D#2000-02-29")]
    #[case(4_102_444_800, "D#2100-01-01")]
    fn format_date_when_seconds_since_epoch_then_iso_calendar_date(
        #[case] secs: u64,
        #[case] expected: &str,
    ) {
        assert_eq!(format_date("D", secs), expected);
    }

    #[test]
    fn format_date_when_seconds_within_a_day_then_truncates_to_that_day() {
        assert_eq!(format_date("D", SECS_PER_DAY - 1), "D#1970-01-01");
    }

    #[test]
    fn format_date_when_ldate_then_uses_its_own_prefix() {
        assert_eq!(format_date("LDATE", 1_705_276_800), "LDATE#2024-01-15");
    }

    #[rstest]
    #[case(0, "TOD#00:00:00")]
    #[case(52_200_000, "TOD#14:30:00")]
    #[case(86_399_999, "TOD#23:59:59.999")]
    fn format_time_of_day_when_milliseconds_then_clock_time(
        #[case] ms: u64,
        #[case] expected: &str,
    ) {
        assert_eq!(format_time_of_day("TOD", ms), expected);
    }

    #[test]
    fn format_time_of_day_when_past_midnight_then_wraps_within_the_day() {
        assert_eq!(
            format_time_of_day("TOD", MS_PER_DAY + 1_000),
            "TOD#00:00:01"
        );
    }

    #[test]
    fn format_time_of_day_when_ltod_then_uses_its_own_prefix() {
        assert_eq!(format_time_of_day("LTOD", 52_200_000), "LTOD#14:30:00");
    }

    #[rstest]
    #[case(0, "DT#1970-01-01-00:00:00")]
    #[case(1_705_329_000, "DT#2024-01-15-14:30:00")]
    fn format_date_and_time_when_seconds_since_epoch_then_date_and_clock_time(
        #[case] secs: u64,
        #[case] expected: &str,
    ) {
        assert_eq!(format_date_and_time("DT", secs), expected);
    }

    #[test]
    fn format_date_and_time_when_ldt_then_uses_its_own_prefix() {
        assert_eq!(
            format_date_and_time("LDT", 1_705_329_000),
            "LDT#2024-01-15-14:30:00"
        );
    }
}
