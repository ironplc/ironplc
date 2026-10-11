//! The elementary types as numbers, for tests whose expected value comes from
//! a Rust oracle rather than from a table.
//!
//! A [`Scalar`] says how one elementary type holds a number: its width and
//! signedness, or that it is a real. A [`Num`] is such a number. Every
//! elementary type other than a string is one, including the durations, dates
//! and times of day, which count milliseconds or seconds (ADR-0021, ADR-0025).
//! That is language meaning, not layout: `TIME_TO_DINT(T#5s)` is 5000.

use ironplc_parser::options::{CompilerOptions, Dialect};
use proptest::prelude::*;
use proptest::sample::select;
use time::macros::datetime;
use time::{Date, Duration, PrimitiveDateTime, Time};

use super::value::Value;

/// A number an elementary variable holds: a count, or a real.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Num {
    Int(i128),
    Real(f64),
}

impl Num {
    /// The number as an integer: a count unchanged, a real without its
    /// fraction.
    pub fn trunc(self) -> i128 {
        match self {
            Num::Int(count) => count,
            Num::Real(real) => real as i128,
        }
    }
}

/// A result that is not what an oracle says it should be.
#[derive(Debug, PartialEq)]
pub struct Mismatch {
    /// The expression, and the inputs it was given.
    pub what: String,
    pub actual: Value,
    pub expected: Value,
}

impl Mismatch {
    /// The mismatch between `actual` and `expected`, if they differ. `what`
    /// is only built for a result that does.
    pub fn of(what: impl FnOnce() -> String, actual: Value, expected: Value) -> Option<Self> {
        (!agree(&actual, &expected)).then(|| Mismatch {
            what: what(),
            actual,
            expected,
        })
    }
}

/// Whether `actual` is `expected`. Two NaNs agree, which `==` denies.
fn agree(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Real(actual), Value::Real(expected)) => {
            actual == expected || (actual.is_nan() && expected.is_nan())
        }
        _ => actual == expected,
    }
}

/// How an elementary type holds a number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Repr {
    Bool,
    Int { bits: u32, signed: bool },
    Real { bits: u32 },
}

/// What a type's number counts, which is how a test writes one to a variable.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Unit {
    /// The number is the value: `BOOL`, an integer, a bit string or a real.
    Itself,
    /// Milliseconds: `TIME`, `LTIME`.
    DurationMs,
    /// Milliseconds since midnight: `TOD`, `LTOD`.
    TimeOfDayMs,
    /// Seconds since 1970-01-01, a whole number of days: `DATE`, `LDATE`.
    DateSeconds,
    /// Seconds since 1970-01-01: `DT`, `LDT`.
    DateAndTimeSeconds,
}

/// An elementary type other than a string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scalar {
    /// The type's name, as a declaration and a conversion function spell it.
    pub name: &'static str,
    pub repr: Repr,
    unit: Unit,
}

const fn scalar(name: &'static str, repr: Repr, unit: Unit) -> Scalar {
    Scalar { name, repr, unit }
}

const fn int(name: &'static str, bits: u32, signed: bool, unit: Unit) -> Scalar {
    scalar(name, Repr::Int { bits, signed }, unit)
}

pub const BOOL: Scalar = scalar("BOOL", Repr::Bool, Unit::Itself);
pub const SINT: Scalar = int("SINT", 8, true, Unit::Itself);
pub const INT: Scalar = int("INT", 16, true, Unit::Itself);
pub const DINT: Scalar = int("DINT", 32, true, Unit::Itself);
pub const LINT: Scalar = int("LINT", 64, true, Unit::Itself);
pub const USINT: Scalar = int("USINT", 8, false, Unit::Itself);
pub const UINT: Scalar = int("UINT", 16, false, Unit::Itself);
pub const UDINT: Scalar = int("UDINT", 32, false, Unit::Itself);
pub const ULINT: Scalar = int("ULINT", 64, false, Unit::Itself);
pub const REAL: Scalar = scalar("REAL", Repr::Real { bits: 32 }, Unit::Itself);
pub const LREAL: Scalar = scalar("LREAL", Repr::Real { bits: 64 }, Unit::Itself);
pub const BYTE: Scalar = int("BYTE", 8, false, Unit::Itself);
pub const WORD: Scalar = int("WORD", 16, false, Unit::Itself);
pub const DWORD: Scalar = int("DWORD", 32, false, Unit::Itself);
pub const LWORD: Scalar = int("LWORD", 64, false, Unit::Itself);
pub const TIME: Scalar = int("TIME", 32, true, Unit::DurationMs);
pub const DATE: Scalar = int("DATE", 32, false, Unit::DateSeconds);
pub const TOD: Scalar = int("TOD", 32, false, Unit::TimeOfDayMs);
pub const DT: Scalar = int("DT", 32, false, Unit::DateAndTimeSeconds);
pub const LTIME: Scalar = int("LTIME", 64, true, Unit::DurationMs);
pub const LDATE: Scalar = int("LDATE", 64, false, Unit::DateSeconds);
pub const LTOD: Scalar = int("LTOD", 64, false, Unit::TimeOfDayMs);
pub const LDT: Scalar = int("LDT", 64, false, Unit::DateAndTimeSeconds);

/// The integer types.
pub const INTEGERS: [Scalar; 8] = [SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT];
/// The bit string types wider than one bit.
pub const BIT_STRINGS: [Scalar; 4] = [BYTE, WORD, DWORD, LWORD];
/// The duration, date and time types, long forms included.
pub const TEMPORALS: [Scalar; 8] = [TIME, DATE, TOD, DT, LTIME, LDATE, LTOD, LDT];

/// Every elementary type other than a string.
pub fn scalars() -> Vec<Scalar> {
    [
        &[BOOL][..],
        &INTEGERS,
        &[REAL, LREAL],
        &BIT_STRINGS,
        &TEMPORALS,
    ]
    .concat()
}

/// The options every [`Scalar`] can be declared under: edition 3, which adds
/// the long duration, date and time types and no extension.
pub fn options() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

const EPOCH: PrimitiveDateTime = datetime!(1970-01-01 0:00);
const SECONDS_PER_DAY: i128 = 86_400;
const MS_PER_DAY: i128 = 86_400_000;
/// The last second a 32-bit date and time holds, and past it for a long one.
const LAST_SECOND: i128 = u32::MAX as i128;
const LAST_LONG_SECOND: i128 = 250_000_000_000;

impl Scalar {
    /// Whether the type is a duration, date or time.
    pub fn is_temporal(self) -> bool {
        self.unit != Unit::Itself
    }

    /// The integer type that holds the same count: itself for an integer, and
    /// for a duration, date or time the integer its count converts to
    /// unchanged.
    pub fn carrier(self) -> Scalar {
        match (self.bits(), self.repr) {
            (64, Repr::Int { signed: true, .. }) => LINT,
            (64, _) => ULINT,
            (_, Repr::Int { signed: true, .. }) => DINT,
            (_, _) => UDINT,
        }
    }

    /// `value` wrapped to the type's storage width: the low bits, sign
    /// extended for a signed type and zero extended for an unsigned one.
    pub fn store(self, value: i128) -> i128 {
        match self.repr {
            Repr::Int { bits, signed } => wrap(value, bits, signed),
            Repr::Bool | Repr::Real { .. } => value,
        }
    }

    /// `value` wrapped to the width the type is operated on at: 32 bits for a
    /// type that fits, otherwise 64 (ADR-0001).
    pub fn operate(self, value: i128) -> i128 {
        match self.repr {
            Repr::Int { bits, signed } => wrap(value, operation_bits(bits), signed),
            Repr::Bool | Repr::Real { .. } => value,
        }
    }

    /// The width the type is operated on at.
    pub fn operation_bits(self) -> u32 {
        match self.repr {
            Repr::Int { bits, .. } | Repr::Real { bits } => operation_bits(bits),
            Repr::Bool => 32,
        }
    }

    /// The type's storage width.
    pub fn bits(self) -> u32 {
        match self.repr {
            Repr::Int { bits, .. } | Repr::Real { bits } => bits,
            Repr::Bool => 1,
        }
    }

    /// `number` as a variable of this type holds it. A number the type holds
    /// as a count becomes the duration, date or time that count stands for.
    pub fn value(self, number: Num) -> Value {
        match (self.unit, self.repr, number) {
            (_, Repr::Bool, Num::Int(count)) => Value::Bool(count != 0),
            (Unit::Itself, _, Num::Int(count)) => Value::Int(count),
            (_, _, Num::Real(real)) => Value::Real(real),
            (Unit::DurationMs, _, Num::Int(count)) => milliseconds(count).into(),
            (Unit::TimeOfDayMs, _, Num::Int(count)) => {
                (Time::MIDNIGHT + milliseconds(count)).into()
            }
            (Unit::DateSeconds, _, Num::Int(count)) => (EPOCH + seconds(count)).date().into(),
            (Unit::DateAndTimeSeconds, _, Num::Int(count)) => (EPOCH + seconds(count)).into(),
        }
    }

    /// The least and the greatest count the type holds. A real holds no
    /// count, and gives zero for both.
    pub fn range(self) -> (i128, i128) {
        match self.repr {
            Repr::Bool => (0, 1),
            Repr::Int { bits, signed: true } => (-(1 << (bits - 1)), (1 << (bits - 1)) - 1),
            Repr::Int {
                bits,
                signed: false,
            } => (0, (1 << bits) - 1),
            Repr::Real { .. } => (0, 0),
        }
    }

    /// The value a variable of the type starts with when its declaration
    /// gives none.
    pub fn zero(self) -> Num {
        match self.repr {
            Repr::Bool | Repr::Int { .. } => Num::Int(0),
            Repr::Real { .. } => Num::Real(0.0),
        }
    }

    /// The literal that writes `number` as a value of this type, or `None`
    /// for an infinity or a NaN, which no literal writes.
    pub fn literal(self, number: Num) -> Option<String> {
        let typed = |text: String| format!("{}#{text}", self.name);
        match self.value(number) {
            Value::Bool(truth) => Some(truth.to_string().to_uppercase()),
            Value::Int(count) if BIT_STRINGS.contains(&self) => Some(format!("16#{count:X}")),
            Value::Int(count) => Some(count.to_string()),
            Value::Real(real) => real.is_finite().then(|| real_literal(real, self.bits())),
            Value::Duration(_) => Some(typed(format!("{}ms", number.trunc()))),
            Value::Date(date) => Some(typed(date_text(date))),
            Value::TimeOfDay(time) => Some(typed(time_text(time))),
            Value::DateAndTime(both) => Some(typed(format!(
                "{}-{}",
                date_text(both.date()),
                time_text(both.time())
            ))),
            Value::Str(_) => None,
        }
    }

    /// The values that sit at the type's limits: zero, the ends of its range,
    /// and for a real a NaN, the infinities and the values a conversion to an
    /// integer rounds, wraps or saturates. A `REAL`'s values are exact in 32
    /// bits.
    pub fn edges(self) -> Vec<Num> {
        match (self.unit, self.repr) {
            (_, Repr::Bool) => ints(&[0, 1]),
            (_, Repr::Real { bits: 32 }) => REAL_EDGES
                .iter()
                .map(|&real| Num::Real(f64::from(real as f32)))
                .chain([
                    Num::Real(f64::from(f32::MAX)),
                    Num::Real(f64::from(f32::MIN)),
                ])
                .collect(),
            (_, Repr::Real { .. }) => REAL_EDGES
                .iter()
                .map(|&real| Num::Real(real))
                .chain([Num::Real(f64::MAX), Num::Real(f64::MIN)])
                .collect(),
            (Unit::Itself | Unit::DurationMs, Repr::Int { .. }) => {
                let (least, greatest) = self.range();
                let mut edges: Vec<i128> = [0, 1, 2, 3, 100, -1, -2]
                    .iter()
                    .map(|&edge| self.store(edge))
                    .chain([least, least + 1, greatest, greatest - 1])
                    .collect();
                edges.sort_unstable();
                edges.dedup();
                ints(&edges)
            }
            (Unit::TimeOfDayMs, _) => ints(&[0, 1, MS_PER_DAY / 2, MS_PER_DAY - 1]),
            (Unit::DateSeconds, _) => ints(&[0, SECONDS_PER_DAY, self.last_day()]),
            (Unit::DateAndTimeSeconds, _) => ints(&[0, 1, SECONDS_PER_DAY, self.last_second()]),
        }
    }

    /// Any value of the type, weighted toward its [`edges`](Self::edges).
    pub fn any(self) -> BoxedStrategy<Num> {
        let edges = select(self.edges());
        match (self.unit, self.repr) {
            (_, Repr::Bool) => edges.boxed(),
            (_, Repr::Real { bits: 32 }) => prop_oneof![
                edges,
                (-70_000f32..70_000f32).prop_map(|real| Num::Real(real.into())),
                (-1e10f32..1e10f32).prop_map(|real| Num::Real(real.into())),
                proptest::num::f32::NORMAL.prop_map(|real| Num::Real(real.into())),
            ]
            .boxed(),
            (_, Repr::Real { .. }) => prop_oneof![
                edges,
                (-70_000f64..70_000f64).prop_map(Num::Real),
                (-1e20f64..1e20f64).prop_map(Num::Real),
                proptest::num::f64::NORMAL.prop_map(Num::Real),
            ]
            .boxed(),
            (Unit::Itself | Unit::DurationMs, Repr::Int { bits, signed }) => prop_oneof![
                edges,
                (-200i128..200).prop_map(move |small| Num::Int(wrap(small, bits, signed))),
                any::<i128>().prop_map(move |raw| Num::Int(wrap(raw, bits, signed))),
            ]
            .boxed(),
            (Unit::TimeOfDayMs, _) => (0..MS_PER_DAY).prop_map(Num::Int).boxed(),
            (Unit::DateSeconds, _) => (0..=self.last_day() / SECONDS_PER_DAY)
                .prop_map(|day| Num::Int(day * SECONDS_PER_DAY))
                .boxed(),
            (Unit::DateAndTimeSeconds, _) => {
                prop_oneof![edges, (0..=self.last_second()).prop_map(Num::Int)].boxed()
            }
        }
    }

    /// Any value of the type other than zero, for a divisor.
    pub fn any_nonzero(self) -> BoxedStrategy<Num> {
        self.any()
            .prop_filter("a divisor is not zero", |&number| number != Num::Int(0))
            .boxed()
    }

    /// The last second the type's count holds that a test can write as a date
    /// and time.
    fn last_second(self) -> i128 {
        if self.bits() == 64 {
            LAST_LONG_SECOND
        } else {
            LAST_SECOND
        }
    }

    /// The first second of the last whole day before
    /// [`last_second`](Self::last_second).
    fn last_day(self) -> i128 {
        self.last_second() / SECONDS_PER_DAY * SECONDS_PER_DAY - SECONDS_PER_DAY
    }
}

/// The reals a conversion to an integer rounds, wraps or saturates.
const REAL_EDGES: [f64; 19] = [
    0.0,
    -0.0,
    0.5,
    -0.5,
    1.5,
    -1.5,
    2.5,
    -2.5,
    255.75,
    -129.25,
    40_000.5,
    -40_000.5,
    3e9,
    -3e9,
    1e19,
    -1e19,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
];

/// `real` as a real literal: digits either side of a point, then an optional
/// exponent. The digits are the fewest that give back the same value at
/// `bits` bits.
fn real_literal(real: f64, bits: u32) -> String {
    let shortest = if bits == 32 {
        format!("{:?}", real as f32)
    } else {
        format!("{real:?}")
    };
    match shortest.split_once('e') {
        Some((digits, exponent)) if !digits.contains('.') => format!("{digits}.0e{exponent}"),
        _ => shortest,
    }
}

/// `date` as the text of a date literal.
fn date_text(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// `time` as the text of a time of day literal.
fn time_text(time: Time) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        time.hour(),
        time.minute(),
        time.second(),
        time.millisecond()
    )
}

fn ints(counts: &[i128]) -> Vec<Num> {
    counts.iter().map(|&count| Num::Int(count)).collect()
}

/// A duration of `count` milliseconds; every count a test writes fits.
fn milliseconds(count: i128) -> Duration {
    Duration::milliseconds(count as i64)
}

/// A duration of `count` seconds; every count a test writes fits.
fn seconds(count: i128) -> Duration {
    Duration::seconds(count as i64)
}

/// The width a type of `bits` bits is operated on at (ADR-0001).
fn operation_bits(bits: u32) -> u32 {
    if bits <= 32 {
        32
    } else {
        64
    }
}

/// The low `bits` bits of `value`, sign extended if `signed`.
pub fn wrap(value: i128, bits: u32, signed: bool) -> i128 {
    let modulus = 1i128 << bits;
    let low = value.rem_euclid(modulus);
    if signed && low >= modulus / 2 {
        low - modulus
    } else {
        low
    }
}
