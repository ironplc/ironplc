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

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct TimeOfDayLiteral {
    value: Time,
}

impl TimeOfDayLiteral {
    pub fn new(value: Time) -> Self {
        Self { value }
    }
}

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct DateLiteral {
    value: Date,
}

impl DateLiteral {
    pub fn new(value: Date) -> Self {
        Self { value }
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
}
