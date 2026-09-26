use std::fmt;
use time::{
    convert::{Day, Hour, Minute, Second},
    Date, Duration, PrimitiveDateTime, Time,
};

use crate::{
    common::{ElementaryTypeName, FixedPoint},
    core::SourceSpan,
};

const SECOND_PER_DAY: u64 = Second::per(Day) as u64;
const SECOND_PER_HOUR: u64 = Second::per(Hour) as u64;
const SECOND_PER_MINUTE: u64 = Second::per(Minute) as u64;

/// The count a temporal literal holds, together with the storage its own type
/// gives that count.
///
/// A temporal value is an integer count in a fixed unit — milliseconds for a
/// duration or a time of day, seconds since 1970-01-01 for a date or a
/// date-and-time — and the literal's type decides how many bits hold it and
/// whether they are signed. Answering all three together is what lets one
/// range check serve every family: the caller asks whether `count` fits
/// `bits` of the stated signedness and needs to know nothing else about dates
/// or durations.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StoredCount {
    /// The count, in the unit the type stores.
    ///
    /// Wider than any storage so that a value the storage cannot hold arrives
    /// intact to be judged, rather than having been truncated on the way.
    pub count: i128,
    /// How many bits hold it: 32 for the short member, 64 for the long one.
    pub bits: u32,
    /// Whether those bits are signed. A duration is signed because it can be
    /// negative (ADR-0021); the calendar types are unsigned counts from the
    /// epoch (ADR-0025).
    pub signed: bool,
}

impl TemporalWidth {
    /// How many bits this width holds.
    pub fn bits(&self) -> u32 {
        match self {
            TemporalWidth::Short => 32,
            TemporalWidth::Long => 64,
        }
    }
}

/// Which member of a temporal family a literal names: the 32-bit type or the
/// 64-bit one.
///
/// IEC 61131-3 pairs each temporal type with a wider one -- `TIME` with
/// `LTIME`, `DATE` with `LDATE`, `TIME_OF_DAY` with `LTIME_OF_DAY`,
/// `DATE_AND_TIME` with `LDATE_AND_TIME` -- and a literal's prefix says which
/// one it is: `T#1h` is a `TIME` and `LTIME#1h` an `LTIME`.
///
/// The width belongs on the literal and not only on the declaration it
/// initializes, for the reason [`CharacterStringLiteral::width`] gives: a
/// literal also appears in statement bodies, where there is no declaration to
/// borrow it from. Without it every temporal literal resolved to the 32-bit
/// type, which held a 64-bit literal to a 32-bit range (issue #1560).
///
/// [`CharacterStringLiteral::width`]: crate::common::CharacterStringLiteral::width
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TemporalWidth {
    /// The 32-bit member: `TIME`, `DATE`, `TIME_OF_DAY`, `DATE_AND_TIME`.
    Short,
    /// The 64-bit member: `LTIME`, `LDATE`, `LTIME_OF_DAY`, `LDATE_AND_TIME`.
    Long,
}

// See section 2.2.2
#[derive(Debug, PartialEq, Clone)]
pub struct DurationLiteral {
    pub span: SourceSpan,
    pub interval: Duration,
    /// The width the source spelled, which is what selects the prefix:
    /// `TIME#`/`T#` for the 32-bit type, `LTIME#` for the 64-bit one.
    pub width: TemporalWidth,
}

impl DurationLiteral {
    /// Creates a literal spanning `span` and measuring `interval`.
    ///
    /// Every constructor funnels through here so that what a duration literal
    /// is made of is stated once. The width defaults to the 32-bit member of
    /// the family, as [`CharacterStringLiteral::new`] defaults to `STRING`;
    /// a caller that knows better says so with
    /// [`with_width`](Self::with_width).
    ///
    /// [`CharacterStringLiteral::new`]: crate::common::CharacterStringLiteral::new
    pub fn new(span: SourceSpan, interval: Duration) -> Self {
        Self {
            span,
            interval,
            width: TemporalWidth::Short,
        }
    }

    /// Returns the literal with `width` recorded as the member of the family
    /// its prefix named.
    pub fn with_width(mut self, width: TemporalWidth) -> Self {
        self.width = width;
        self
    }

    /// The IEC 61131-3 type this literal is: the duration type its prefix named.
    ///
    /// A literal states its own type, so it is checked against that type's
    /// range wherever it is written, the way a prefixed integer literal is
    /// (`INT#40000` is not an `INT` whatever it is stored into).
    pub fn type_name(&self) -> ElementaryTypeName {
        match self.width {
            TemporalWidth::Short => ElementaryTypeName::TIME,
            TemporalWidth::Long => ElementaryTypeName::LTIME,
        }
    }

    /// The millisecond count this literal holds and the storage its type gives
    /// it.
    ///
    /// A duration is signed: subtracting a later time from an earlier one
    /// gives a negative result (ADR-0021).
    pub fn stored_count(&self) -> StoredCount {
        StoredCount {
            count: self.interval.whole_milliseconds(),
            bits: self.width.bits(),
            signed: true,
        }
    }

    /// Creates a literal of `value` units, where one unit is `seconds_per_unit`
    /// seconds and `whole_units` builds the whole part.
    ///
    /// A fixed-point literal carries its whole part and a femtosecond
    /// fraction separately, and days, hours and minutes each scale that
    /// fraction by their own unit before it becomes a duration. Only the unit
    /// differs between them, so only the unit is passed in.
    fn from_whole_unit(
        value: FixedPoint,
        whole_units: fn(i64) -> Duration,
        seconds_per_unit: u64,
    ) -> Self {
        let whole = whole_units(value.whole as i64);

        // `femptos / FRACTIONAL_UNITS` is the fraction of one unit, so the
        // fraction in microseconds is
        //
        //     femptos / 1e15 * seconds_per_unit * 1e6 == femptos * seconds_per_unit / 1e9
        //
        // computed in `u128` because the numerator does not fit a `u64`: half
        // a day is 5e14 femtos times 86,400 seconds, which is 4.3e19 against a
        // `u64::MAX` of 1.8e19. The quotient is at most 8.64e10 microseconds,
        // one whole unit's worth, so it always fits the `i64` a `Duration`
        // takes.
        let fraction = Duration::microseconds(
            (u128::from(value.femptos) * u128::from(seconds_per_unit)
                / (FixedPoint::FRACTIONAL_UNITS as u128 / 1_000_000)) as i64,
        );

        Self::new(value.span, whole + fraction)
    }

    /// Create a new `DurationLiteral` with the given number of days.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::days(FixedPoint::parse("1").unwrap()).interval, Duration::days(1));
    /// ```
    pub fn days(days: FixedPoint) -> Self {
        Self::from_whole_unit(days, Duration::days, SECOND_PER_DAY)
    }

    /// Create a new `DurationLiteral` with the given number of hours.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::hours(FixedPoint::parse("1").unwrap()).interval, Duration::hours(1));
    /// assert_eq!(DurationLiteral::hours(FixedPoint::parse("1.5").unwrap()).interval, Duration::minutes(90));
    /// ```
    pub fn hours(hours: FixedPoint) -> Self {
        Self::from_whole_unit(hours, Duration::hours, SECOND_PER_HOUR)
    }

    /// Create a new `DurationLiteral` with the given number of minutes.
    ///
    /// ```rust
    /// use ironplc_dsl::common::FixedPoint;
    /// use ironplc_dsl::time::DurationLiteral;
    /// use time::Duration;
    /// assert_eq!(DurationLiteral::minutes(FixedPoint::parse("1").unwrap()).interval, Duration::minutes(1));
    /// assert_eq!(DurationLiteral::minutes(FixedPoint::parse("1.5").unwrap()).interval, Duration::seconds(90));
    /// ```
    pub fn minutes(minutes: FixedPoint) -> Self {
        Self::from_whole_unit(minutes, Duration::minutes, SECOND_PER_MINUTE)
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
        Self::new(seconds.span, whole_seconds + fraction_seconds)
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
        Self::new(
            millis.span,
            whole_seconds + whole_milliseconds + fraction_nanoseconds,
        )
    }

    pub fn plus(&self, other: DurationLiteral) -> Self {
        Self::new(
            SourceSpan::join(&self.span, &other.span),
            self.interval + other.interval,
        )
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
    /// The literal's position in the source text.
    pub span: SourceSpan,
    /// The width the source spelled: `TIME_OF_DAY#`/`TOD#` for the 32-bit
    /// type, `LTIME_OF_DAY#`/`LTOD#` for the 64-bit one.
    pub width: TemporalWidth,
}

impl TimeOfDayLiteral {
    pub fn new(value: Time) -> Self {
        Self {
            value,
            span: SourceSpan::default(),
            width: TemporalWidth::Short,
        }
    }

    /// Returns the literal with `width` recorded as the member of the family
    /// its prefix named.
    pub fn with_width(mut self, width: TemporalWidth) -> Self {
        self.width = width;
        self
    }

    /// The IEC 61131-3 type this literal is: the time of day type its prefix named.
    ///
    /// A literal states its own type, so it is checked against that type's
    /// range wherever it is written, the way a prefixed integer literal is
    /// (`INT#40000` is not an `INT` whatever it is stored into).
    pub fn type_name(&self) -> ElementaryTypeName {
        match self.width {
            TemporalWidth::Short => ElementaryTypeName::TimeOfDay,
            TemporalWidth::Long => ElementaryTypeName::LTimeOfDay,
        }
    }

    /// The millisecond-since-midnight count this literal holds and the storage
    /// its type gives it.
    ///
    /// The count is unsigned and bounded by 86,399,999 by construction, so it
    /// fits either width; the range check is vacuous rather than absent, so
    /// that a bound which stopped holding would be reported rather than
    /// silently truncated.
    pub fn stored_count(&self) -> StoredCount {
        StoredCount {
            count: i128::from(self.whole_milliseconds()),
            bits: self.width.bits(),
            signed: false,
        }
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

/// The number of seconds from the Unix epoch to midnight on `date`, negative
/// for a date before the epoch.
///
/// Shared by the two date literal types so that a date and a date-and-time
/// agree on where the epoch is and what a day is worth.
fn seconds_to_midnight(date: &Date) -> i64 {
    const UNIX_EPOCH_JULIAN_DAY: i32 = 2_440_588; // 1970-01-01
    let days = i64::from(date.to_julian_day() - UNIX_EPOCH_JULIAN_DAY);
    days * i64::from(Second::per(Day))
}

// See section 2.2.3
#[derive(Debug, PartialEq, Clone)]
pub struct DateLiteral {
    pub value: Date,
    /// The literal's position in the source text.
    pub span: SourceSpan,
    /// The width the source spelled: `DATE#`/`D#` for the 32-bit type,
    /// `LDATE#` for the 64-bit one.
    pub width: TemporalWidth,
}

impl DateLiteral {
    pub fn new(value: Date) -> Self {
        Self {
            value,
            span: SourceSpan::default(),
            width: TemporalWidth::Short,
        }
    }

    /// Returns the literal with `width` recorded as the member of the family
    /// its prefix named.
    pub fn with_width(mut self, width: TemporalWidth) -> Self {
        self.width = width;
        self
    }

    /// The IEC 61131-3 type this literal is: the date type its prefix named.
    ///
    /// A literal states its own type, so it is checked against that type's
    /// range wherever it is written, the way a prefixed integer literal is
    /// (`INT#40000` is not an `INT` whatever it is stored into).
    pub fn type_name(&self) -> ElementaryTypeName {
        match self.width {
            TemporalWidth::Short => ElementaryTypeName::DATE,
            TemporalWidth::Long => ElementaryTypeName::LDATE,
        }
    }

    /// The epoch-second count this literal holds and the storage its type
    /// gives it.
    ///
    /// The count is unsigned (ADR-0025), so a date before 1970-01-01 has
    /// nowhere to go at either width.
    pub fn stored_count(&self) -> StoredCount {
        StoredCount {
            count: i128::from(self.seconds_since_epoch()),
            bits: self.width.bits(),
            signed: false,
        }
    }

    /// Returns the year, month, day from the literal.
    pub fn ymd(&self) -> (i32, u8, u8) {
        let year = self.value.year();
        let month = self.value.month();
        let day = self.value.day();
        (year, month.into(), day)
    }

    /// Returns seconds since the Unix epoch (1970-01-01).
    ///
    /// The IEC 61131-3 DATE type is stored as a u32 count of seconds since
    /// 1970-01-01, matching the CODESYS/Beckhoff industry standard. The
    /// resolution is logically 1 day but the storage unit is seconds for
    /// compatibility with DATE_AND_TIME.
    ///
    /// The count returned is the literal's own, which is not always a value
    /// the storage can hold: it is negative for a date before the epoch and
    /// beyond `u32::MAX` for one after 2106-02-07. It is computed wider than
    /// the storage so that those dates arrive at the caller to be judged
    /// rather than trapping here.
    pub fn seconds_since_epoch(&self) -> i64 {
        seconds_to_midnight(&self.value)
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
    /// The literal's position in the source text.
    pub span: SourceSpan,
    /// The width the source spelled: `DATE_AND_TIME#`/`DT#` for the 32-bit
    /// type, `LDATE_AND_TIME#`/`LDT#` for the 64-bit one.
    pub width: TemporalWidth,
}

impl DateAndTimeLiteral {
    pub fn new(value: PrimitiveDateTime) -> Self {
        Self {
            value,
            span: SourceSpan::default(),
            width: TemporalWidth::Short,
        }
    }

    /// Returns the literal with `width` recorded as the member of the family
    /// its prefix named.
    pub fn with_width(mut self, width: TemporalWidth) -> Self {
        self.width = width;
        self
    }

    /// The IEC 61131-3 type this literal is: the date and time type its prefix named.
    ///
    /// A literal states its own type, so it is checked against that type's
    /// range wherever it is written, the way a prefixed integer literal is
    /// (`INT#40000` is not an `INT` whatever it is stored into).
    pub fn type_name(&self) -> ElementaryTypeName {
        match self.width {
            TemporalWidth::Short => ElementaryTypeName::DateAndTime,
            TemporalWidth::Long => ElementaryTypeName::LDateAndTime,
        }
    }

    /// The epoch-second count this literal holds and the storage its type
    /// gives it.
    ///
    /// As with [`DateLiteral::stored_count`], the count is unsigned.
    pub fn stored_count(&self) -> StoredCount {
        StoredCount {
            count: i128::from(self.seconds_since_epoch()),
            bits: self.width.bits(),
            signed: false,
        }
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

    /// Returns seconds since the Unix epoch (1970-01-01 00:00:00).
    ///
    /// The IEC 61131-3 DATE_AND_TIME type is stored as a u32 count of seconds
    /// since 1970-01-01, matching the CODESYS/Beckhoff industry standard.
    /// Resolution is 1 second.
    ///
    /// As with [`DateLiteral::seconds_since_epoch`], the count is the
    /// literal's own and may lie outside what the storage holds.
    pub fn seconds_since_epoch(&self) -> i64 {
        let (h, m, s, _micro) = self.hmsm();
        let tod_secs = i64::from(h) * i64::from(Second::per(Hour))
            + i64::from(m) * i64::from(Second::per(Minute))
            + i64::from(s);
        seconds_to_midnight(&self.value.date()) + tod_secs
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Date, Month, PrimitiveDateTime, Time};

    #[test]
    fn days_when_one_day_then_correct_duration() {
        let fp = FixedPoint::parse("1").unwrap();
        let dur = DurationLiteral::days(fp);
        assert_eq!(dur.interval, Duration::days(1));
    }

    #[test]
    fn hours_when_one_hour_then_correct_duration() {
        let fp = FixedPoint::parse("1").unwrap();
        let dur = DurationLiteral::hours(fp);
        assert_eq!(dur.interval, Duration::hours(1));
    }

    #[test]
    fn minutes_when_one_minute_then_correct_duration() {
        let fp = FixedPoint::parse("1").unwrap();
        let dur = DurationLiteral::minutes(fp);
        assert_eq!(dur.interval, Duration::minutes(1));
    }

    #[test]
    fn plus_when_two_durations_then_sum() {
        let a = DurationLiteral::seconds(FixedPoint::parse("1").unwrap());
        let b = DurationLiteral::seconds(FixedPoint::parse("2").unwrap());
        let result = a.plus(b);
        assert_eq!(result.interval, Duration::seconds(3));
    }

    #[test]
    fn display_when_duration_then_formats_as_time_ms() {
        let dur = DurationLiteral::seconds(FixedPoint::parse("2").unwrap());
        assert_eq!(format!("{dur}"), "TIME#2000ms");
    }

    #[test]
    fn display_when_time_of_day_then_formats_as_tod() {
        let tod = TimeOfDayLiteral::new(Time::from_hms(14, 30, 0).unwrap());
        assert_eq!(format!("{tod}"), "TIME_OF_DAY#14:30:00");
    }

    #[test]
    fn display_when_date_then_formats_as_date() {
        let date = DateLiteral::new(Date::from_calendar_date(2025, Month::March, 15).unwrap());
        assert_eq!(format!("{date}"), "DATE#2025-03-15");
    }

    #[test]
    fn display_when_date_and_time_then_formats_as_dt() {
        let dt = DateAndTimeLiteral::new(PrimitiveDateTime::new(
            Date::from_calendar_date(2025, Month::January, 1).unwrap(),
            Time::from_hms(12, 0, 0).unwrap(),
        ));
        assert_eq!(format!("{dt}"), "DATE_AND_TIME#2025-01-01-12:00:00");
    }
}
