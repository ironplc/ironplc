//! Reading a typed value out of a variable slot.
//!
//! This module is the one place the end-to-end tests know how the VM encodes
//! a value. A test states the type it expects (`i32`, `f64`, `Duration`,
//! `Date`, ...) and compares against a value of that type, never against the
//! slot's bits or a count in the VM's storage unit.

use std::fmt::{Debug, Display};

use ironplc_container::debug_section::{function_id, iec_type_tag};
use ironplc_container::Container;
use ironplc_vm::Slot;
use time::macros::datetime;
use time::{Date, Duration, PrimitiveDateTime, Time};

/// A Rust type that an end-to-end assertion reads out of a variable slot.
///
/// The type decides how the slot's bits are interpreted, so a test states
/// it once (`assert_run::<f32>`) rather than choosing a reader per slot.
pub trait SlotValue: Copy + PartialEq + Debug {
    /// Reads the value held in `slot` by a variable whose debug type tag is
    /// `tag`.
    fn from_slot(slot: Slot, tag: u8) -> Self;
}

/// A [`SlotValue`] that `assert_run_near` can compare within a tolerance.
pub trait NearSlotValue: SlotValue + PartialOrd + Display {
    /// The absolute difference between `self` and `other`.
    fn distance(self, other: Self) -> Self;
}

macro_rules! impl_number {
    ($($t:ty => $read:ident),*) => {$(
        impl SlotValue for $t {
            fn from_slot(slot: Slot, _tag: u8) -> Self { slot.$read() }
        }
        impl NearSlotValue for $t {
            fn distance(self, other: Self) -> Self { (self - other).abs() }
        }
    )*};
}

impl_number!(i32 => as_i32, i64 => as_i64, f32 => as_f32, f64 => as_f64);

// Dates and durations. The VM stores durations and times of day as
// milliseconds and dates as seconds since 1970 (ADR-0021, ADR-0025), the short
// types in 32 bits; these four readers turn those counts back into values, so
// no test states a unit.

const EPOCH: PrimitiveDateTime = datetime!(1970-01-01 0:00);

impl SlotValue for Duration {
    fn from_slot(slot: Slot, tag: u8) -> Self {
        assert!(
            matches!(tag, iec_type_tag::TIME | iec_type_tag::LTIME),
            "a Duration is read from a TIME or LTIME variable, not type tag {tag}"
        );
        // A 32-bit TIME is sign-extended into its slot, so one read serves both.
        Duration::milliseconds(slot.as_i64())
    }
}

impl SlotValue for Time {
    fn from_slot(slot: Slot, tag: u8) -> Self {
        let millis = unsigned_count(slot, tag, iec_type_tag::TIME_OF_DAY, iec_type_tag::LTOD);
        Time::MIDNIGHT + Duration::milliseconds(millis)
    }
}

impl SlotValue for Date {
    fn from_slot(slot: Slot, tag: u8) -> Self {
        let seconds = unsigned_count(slot, tag, iec_type_tag::DATE, iec_type_tag::LDATE);
        (EPOCH + Duration::seconds(seconds)).date()
    }
}

impl SlotValue for PrimitiveDateTime {
    fn from_slot(slot: Slot, tag: u8) -> Self {
        let seconds = unsigned_count(slot, tag, iec_type_tag::DATE_AND_TIME, iec_type_tag::LDT);
        EPOCH + Duration::seconds(seconds)
    }
}

/// The unsigned count a date or time of day holds: 32 bits when `tag` is the
/// short type `short`, 64 bits when it is the long type `long`.
fn unsigned_count(slot: Slot, tag: u8, short: u8, long: u8) -> i64 {
    match tag {
        t if t == short => i64::from(slot.as_i32() as u32),
        t if t == long => slot.as_i64(),
        _ => panic!("expected a variable of type tag {short} or {long}, not {tag}"),
    }
}

/// The debug type tag of the program or global variable in slot `index`, or
/// `OTHER` when the container records none.
pub(super) fn type_tag(container: &Container, index: usize) -> u8 {
    container
        .debug_section
        .as_ref()
        .and_then(|debug| {
            debug.var_names.iter().find(|entry| {
                entry.function_id == function_id::GLOBAL_SCOPE
                    && usize::from(entry.var_index.raw()) == index
            })
        })
        .map_or(iec_type_tag::OTHER, |entry| entry.iec_type_tag)
}
