//! The value a test observes, whatever the backend that computed it.
//!
//! A test names a variable and compares what it reads against an IEC value: a
//! number, a string, a duration or a date. Nothing here knows how a backend
//! stores a value; `slot_value` decodes the VM's storage into a [`Value`].

use std::fmt::{Debug, Display};

use ironplc_container::debug_section::iec_type_tag;
use time::{Date, Duration, PrimitiveDateTime, Time};

/// An IEC 61131-3 value as a test observes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    /// Every integer and bit-string type, holding its value exactly.
    Int(i128),
    /// `REAL` widened to `f64` exactly, or `LREAL` unchanged.
    Real(f64),
    /// `STRING` or `WSTRING`, holding the current content.
    Str(String),
    /// `TIME` or `LTIME`.
    Duration(Duration),
    /// `DATE` or `LDATE`.
    Date(Date),
    /// `TIME_OF_DAY` or `LTIME_OF_DAY`.
    TimeOfDay(Time),
    /// `DATE_AND_TIME` or `LDATE_AND_TIME`.
    DateAndTime(PrimitiveDateTime),
}

impl Value {
    /// Whether this value, read from a variable, is `expected`. A `BOOL`
    /// equals the integer 1 when TRUE and 0 when FALSE, as `BOOL_TO_INT`
    /// converts it.
    pub fn matches(&self, expected: &Value) -> bool {
        match (self, expected) {
            (Value::Bool(b), Value::Int(i)) => *i == i128::from(*b),
            _ => self == expected,
        }
    }
}

/// Whether `value` equals `other`; the parameter types fix what `other` is
/// converted into.
fn same<T: PartialEq>(value: &T, other: T) -> bool {
    *value == other
}

macro_rules! from_into {
    ($($t:ty => $variant:ident),* $(,)?) => {$(
        impl From<$t> for Value {
            fn from(value: $t) -> Self {
                Value::$variant(value.into())
            }
        }
        impl PartialEq<$t> for Value {
            fn eq(&self, other: &$t) -> bool {
                match self {
                    Value::$variant(value) => same(value, (*other).into()),
                    _ => false,
                }
            }
        }
    )*};
}

from_into!(
    bool => Bool,
    i8 => Int, i16 => Int, i32 => Int, i64 => Int,
    u8 => Int, u16 => Int, u32 => Int, u64 => Int,
    f32 => Real, f64 => Real,
    Duration => Duration, Date => Date, Time => TimeOfDay, PrimitiveDateTime => DateAndTime,
);

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Str(value.to_string())
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Value::Str(s) if s == other)
    }
}

/// A Rust type that a typed assertion compares a variable's value against.
///
/// The conversion is checked: a value that the type cannot hold without loss,
/// or a value of another family, fails the test rather than being coerced.
pub trait FromValue: Copy + PartialEq + Debug {
    /// Converts `value`, read from a variable whose declared type is `tag`,
    /// or says why it cannot be converted without loss.
    fn from_value(value: &Value, tag: u8) -> Result<Self, String>;
}

/// A [`FromValue`] that `assert_run_near` can compare within a tolerance.
pub trait NearValue: FromValue + PartialOrd + Display {
    /// The absolute difference between `self` and `other`.
    fn distance(self, other: Self) -> Self;
}

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl FromValue for $t {
            fn from_value(value: &Value, _tag: u8) -> Result<Self, String> {
                match value {
                    // A BOOL compares as 1 or 0, as BOOL_TO_INT converts it.
                    Value::Bool(b) => Ok(<$t>::from(*b)),
                    Value::Int(i) => <$t>::try_from(*i)
                        .map_err(|_| format!("{i} does not fit {}", stringify!($t))),
                    other => Err(format!("{other:?} is not an integer")),
                }
            }
        }
    )*};
}

from_int!(i32, i64, u32, u64);

impl FromValue for f32 {
    fn from_value(value: &Value, tag: u8) -> Result<Self, String> {
        match value {
            // Only a REAL holds an f32 exactly; an LREAL would be narrowed.
            Value::Real(r) if tag == iec_type_tag::REAL => Ok(*r as f32),
            Value::Real(_) => Err("an f32 is compared against a REAL, not an LREAL".into()),
            other => Err(format!("{other:?} is not a real")),
        }
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value, _tag: u8) -> Result<Self, String> {
        match value {
            Value::Real(r) => Ok(*r),
            other => Err(format!("{other:?} is not a real")),
        }
    }
}

macro_rules! from_variant {
    ($($t:ty => $variant:ident),*) => {$(
        impl FromValue for $t {
            fn from_value(value: &Value, _tag: u8) -> Result<Self, String> {
                match value {
                    Value::$variant(v) => Ok(*v),
                    other => Err(format!("{other:?} is not a {}", stringify!($variant))),
                }
            }
        }
    )*};
}

from_variant!(
    Duration => Duration,
    Date => Date,
    Time => TimeOfDay,
    PrimitiveDateTime => DateAndTime
);

impl NearValue for f32 {
    fn distance(self, other: Self) -> Self {
        (self - other).abs()
    }
}

impl NearValue for f64 {
    fn distance(self, other: Self) -> Self {
        (self - other).abs()
    }
}
