use std::fmt;
use time::{
    convert::{Day, Hour, Minute, Second},
    Date, Duration, PrimitiveDateTime, Time,
};

use crate::{common::FixedPoint, core::SourceSpan};

const SECOND_PER_DAY: u64 = Second::per(Day) as u64;
const SECOND_PER_HOUR: u64 = Second::per(Hour) as u64;
const SECOND_PER_MINUTE: u64 = Second::per(Minute) as u64;

// See section 2.2.2
#[derive(Debug, PartialEq, Clone)]
pub struct DurationLiteral {
    pub span: SourceSpan,
    pub interval: Duration,
}

impl DurationLiteral {
    /// Create a new `DurationLiteral` with the given number of days.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::days(FixedPoint::parse("1").unwrap()).interval, Duration::days(1));
    /// ```
    pub fn days(days: FixedPoint) -> Self {
        // The whole part is entirely seconds
        let whole_seconds = Duration::days(days.whole as i64);

        // The fraction has both seconds and one part femptoseconds
        let fraction_seconds = Duration::microseconds(
            (days.femptos * SECOND_PER_DAY / FixedPoint::FRACTIONAL_UNITS) as i64,
        );

        Self {
            span: days.span,
            interval: whole_seconds + fraction_seconds,
        }
    }

    /// Create a new `DurationLiteral` with the given number of hours.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1").unwrap()).interval, Duration::seconds(1));
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1.001").unwrap()).interval, Duration::seconds(1) + Duration::milliseconds(1));
    /// ```
    pub fn hours(hours: FixedPoint) -> Self {
        // The whole part is entirely seconds
        let whole_seconds = Duration::hours(hours.whole as i64);

        // The fraction has both seconds and one part femptoseconds
        let fraction_seconds = Duration::microseconds(
            (hours.femptos * SECOND_PER_HOUR / FixedPoint::FRACTIONAL_UNITS) as i64,
        );

        Self {
            span: hours.span,
            interval: whole_seconds + fraction_seconds,
        }
    }

    /// Create a new `DurationLiteral` with the given number of minutes.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1").unwrap()).interval, Duration::seconds(1));
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1.001").unwrap()).interval, Duration::seconds(1) + Duration::milliseconds(1));
    /// ```
    pub fn minutes(minutes: FixedPoint) -> Self {
        // The whole part is entirely seconds
        let whole_seconds = Duration::minutes(minutes.whole as i64);

        // The fraction has both seconds and one part femptoseconds
        let fraction_seconds = Duration::microseconds(
            (minutes.femptos * SECOND_PER_MINUTE / FixedPoint::FRACTIONAL_UNITS) as i64,
        );
        Self {
            span: minutes.span,
            interval: whole_seconds + fraction_seconds,
        }
    }

    /// Create a new `DurationLiteral` with the given number of seconds.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1").unwrap()).interval, Duration::seconds(1));
    /// assert_eq!(DurationLiteral::seconds(FixedPoint::parse("1.001").unwrap()).interval, Duration::seconds(1) + Duration::milliseconds(1));
    /// ```
    pub fn seconds(seconds: FixedPoint) -> Self {
        let whole_seconds = Duration::seconds(seconds.whole as i64);
        let fraction_seconds = Duration::nanoseconds((seconds.femptos / 1_000_000) as i64);
        Self {
            span: seconds.span,
            interval: whole_seconds + fraction_seconds,
        }
    }

    /// Create a new `DurationLiteral` with the given number of milliseconds.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::milliseconds(FixedPoint::parse("1").unwrap()).interval, Duration::milliseconds(1));
    /// assert_eq!(DurationLiteral::milliseconds(FixedPoint::parse("1000").unwrap()).interval, Duration::seconds(1));
    /// assert_eq!(DurationLiteral::milliseconds(FixedPoint::parse("1001").unwrap()).interval, Duration::seconds(1) + Duration::milliseconds(1));
    /// assert_eq!(DurationLiteral::milliseconds(FixedPoint::parse("0.001").unwrap()).interval, Duration::microseconds(1));
    /// ```
    pub fn milliseconds(millis: FixedPoint) -> Self {
        let whole_seconds = Duration::seconds((millis.whole / 1_000) as i64);
        let whole_milliseconds = Duration::milliseconds((millis.whole % 1_000) as i64);

        let fraction_nanoseconds = Duration::nanoseconds((millis.femptos / 1_000_000_000) as i64);
        Self {
            span: millis.span,
            interval: whole_seconds + whole_milliseconds + fraction_nanoseconds,
        }
    }

    pub fn plus(&self, other: DurationLiteral) -> Self {
        DurationLiteral {
            span: SourceSpan::join(&self.span, &other.span),
            interval: self.interval + other.interval,
        }
    }
}

impl fmt::Display for DurationLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TIME#{}ms", self.interval.whole_milliseconds())
    }
}

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct TimeOfDayLiteral {
    value: Time,
}

impl TimeOfDayLiteral {
    pub fn new(value: Time) -> Self {
        Self { value }
    }

    /// Returns the hour, minute, second and microsecond from the literal.
    pub fn hmsm(&self) -> (u8, u8, u8, u32) {
        self.value.as_hms_micro()
    }

    /// Returns milliseconds since midnight as a u32.
    ///
    /// Maximum value is 86_399_999 (23:59:59.999).
    /// Microsecond precision from the underlying Time is truncated to milliseconds.
    pub fn whole_milliseconds(&self) -> u32 {
        let (h, m, s, micro) = self.hmsm();
        (h as u32) * 3_600_000 + (m as u32) * 60_000 + (s as u32) * 1_000 + micro / 1_000
    }
}

impl fmt::Display for TimeOfDayLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, m, s, _) = self.hmsm();
        write!(f, "TIME_OF_DAY#{:02}:{:02}:{:02}", h, m, s)
    }
}

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct DateLiteral {
    pub value: Date,
}

impl DateLiteral {
    pub fn new(value: Date) -> Self {
        Self { value }
    }

    /// Returns the year, month, day from the literal.
    pub fn ymd(&self) -> (i32, u8, u8) {
        let year = self.value.year();
        let month = self.value.month();
        let day = self.value.day();
        (year, month.into(), day)
    }

    /// Returns the number of days since the epoch 0001-01-01 as a u32.
    ///
    /// The IEC 61131-3 DATE type represents calendar dates. We use an
    /// epoch of 0001-01-01 (Julian day 1721426) so that all valid dates
    /// in the Gregorian/proleptic calendar produce non-negative values.
    pub fn days_since_epoch(&self) -> u32 {
        // Julian day of 0001-01-01
        const EPOCH_JULIAN_DAY: i32 = 1_721_426;
        let julian_day = self.value.to_julian_day();
        (julian_day - EPOCH_JULIAN_DAY) as u32
    }
}

impl fmt::Display for DateLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "DATE#{}-{:02}-{:02}", y, m, d)
    }
}

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct DateAndTimeLiteral {
    value: PrimitiveDateTime,
}

impl DateAndTimeLiteral {
    pub fn new(value: PrimitiveDateTime) -> Self {
        Self { value }
    }

    /// Returns the year, month, day from the literal.
    pub fn ymd(&self) -> (i32, u8, u8) {
        let year = self.value.year();
        let month = self.value.month();
        let day = self.value.day();
        (year, month.into(), day)
    }

    /// Returns the hour, minute, second and microsecond from the literal.
    pub fn hmsm(&self) -> (u8, u8, u8, u32) {
        self.value.as_hms_micro()
    }

    /// Returns milliseconds since 0001-01-01 00:00:00 as a u64.
    ///
    /// Combines the date component (days since epoch) with the time-of-day
    /// component (milliseconds since midnight) into a single u64 value.
    pub fn whole_milliseconds(&self) -> u64 {
        const EPOCH_JULIAN_DAY: i32 = 1_721_426;
        let days = (self.value.date().to_julian_day() - EPOCH_JULIAN_DAY) as u64;
        let (h, m, s, micro) = self.hmsm();
        let tod_ms = (h as u64) * 3_600_000
            + (m as u64) * 60_000
            + (s as u64) * 1_000
            + (micro as u64) / 1_000;
        days * 86_400_000 + tod_ms
    }
}

impl fmt::Display for DateAndTimeLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        let (h, min, s, _) = self.hmsm();
        write!(
            f,
            "DATE_AND_TIME#{}-{:02}-{:02}-{:02}:{:02}:{:02}",
            y, m, d, h, min, s
        )
    }
}
