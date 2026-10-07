//! Decoding a VM variable into a [`Value`], and encoding one into a slot.
//!
//! This module is the one place the end-to-end tests know how the VM stores a
//! value: which slot reader a type needs, that a `UDINT` is the low 32 bits,
//! that a duration is a count of milliseconds. Everything above it compares
//! [`Value`]s, so a change to the VM's encoding changes this module and no
//! test.

use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_vm::Slot;
use time::macros::datetime;
use time::{Duration, PrimitiveDateTime, Time};

use super::value::Value;

/// Decodes the value a variable of debug type tag `tag` holds in `slot`.
///
/// A string's content lives in `data_region` at `string_offset`, the offset
/// its `STRING_LAYOUT` entry records.
pub(super) fn decode(slot: Slot, tag: u8, data_region: &[u8], string_offset: Option<u32>) -> Value {
    use iec_type_tag::*;
    match tag {
        BOOL => Value::Bool(slot.as_i32() != 0),
        SINT => Value::Int((slot.as_i32() as i8).into()),
        INT => Value::Int((slot.as_i32() as i16).into()),
        DINT => Value::Int(slot.as_i32().into()),
        LINT => Value::Int(slot.as_i64().into()),
        USINT | BYTE => Value::Int((slot.as_i32() as u8).into()),
        UINT | WORD => Value::Int((slot.as_i32() as u16).into()),
        UDINT | DWORD => Value::Int((slot.as_i32() as u32).into()),
        ULINT | LWORD => Value::Int(slot.as_u64().into()),
        REAL => Value::Real(slot.as_f32().into()),
        LREAL => Value::Real(slot.as_f64()),
        STRING | WSTRING => Value::Str(read_string(
            data_region,
            string_offset.expect("a string variable has a STRING_LAYOUT entry"),
        )),
        // Durations and times of day are milliseconds, dates seconds since
        // 1970 (ADR-0021, ADR-0025). A 32-bit TIME is sign-extended into its
        // slot, so one read serves both widths.
        TIME | LTIME => Value::Duration(Duration::milliseconds(slot.as_i64())),
        TIME_OF_DAY | LTOD => {
            Value::TimeOfDay(Time::MIDNIGHT + Duration::milliseconds(unsigned(slot, tag, LTOD)))
        }
        DATE | LDATE => Value::Date((EPOCH + Duration::seconds(unsigned(slot, tag, LDATE))).date()),
        DATE_AND_TIME | LDT => {
            Value::DateAndTime(EPOCH + Duration::seconds(unsigned(slot, tag, LDT)))
        }
        STRUCT | ARRAY | FB_INSTANCE => {
            panic!("a structure, array or function block is read through a path into it")
        }
        // A named subrange, or a declaration codegen records no type for:
        // the slot holds the value as a signed integer.
        _ => Value::Int(slot.as_i64().into()),
    }
}

/// Encodes `value` as the slot a variable of debug type tag `tag` holds it
/// in, or says why that type cannot hold it.
///
/// A `BOOL` accepts the integer 1 or 0, as `INT_TO_BOOL` converts them. A date
/// or duration never comes from a number: the number would have to be in the
/// VM's storage unit, which is what [`Value`] keeps out of the tests.
pub(super) fn encode(value: &Value, tag: u8) -> Result<Slot, String> {
    use iec_type_tag::*;
    let slot = match (tag, value) {
        (BOOL, Value::Bool(b)) => Slot::from_i32(i32::from(*b)),
        (BOOL, Value::Int(i @ (0 | 1))) => Slot::from_i32(*i as i32),
        (SINT, Value::Int(i)) => Slot::from_i32(fit::<i8>(*i)?.into()),
        (INT, Value::Int(i)) => Slot::from_i32(fit::<i16>(*i)?.into()),
        (DINT, Value::Int(i)) => Slot::from_i32(fit::<i32>(*i)?),
        (LINT, Value::Int(i)) => Slot::from_i64(fit::<i64>(*i)?),
        (USINT | BYTE, Value::Int(i)) => Slot::from_i32(fit::<u8>(*i)?.into()),
        (UINT | WORD, Value::Int(i)) => Slot::from_i32(fit::<u16>(*i)?.into()),
        (UDINT | DWORD, Value::Int(i)) => Slot::from_i32(fit::<u32>(*i)? as i32),
        (ULINT | LWORD, Value::Int(i)) => Slot::from_u64(fit::<u64>(*i)?),
        (REAL, Value::Real(r)) if f64::from(*r as f32) == *r => Slot::from_f32(*r as f32),
        (LREAL, Value::Real(r)) => Slot::from_f64(*r),
        (TIME, Value::Duration(d)) => Slot::from_i32(fit::<i32>(millis(*d)?)?),
        (LTIME, Value::Duration(d)) => Slot::from_i64(fit::<i64>(millis(*d)?)?),
        (TIME_OF_DAY | LTOD, Value::TimeOfDay(t)) => {
            unsigned_slot(millis(*t - Time::MIDNIGHT)?, tag, LTOD)?
        }
        (DATE | LDATE, Value::Date(d)) => {
            unsigned_slot(seconds(d.midnight() - EPOCH)?, tag, LDATE)?
        }
        (DATE_AND_TIME | LDT, Value::DateAndTime(dt)) => {
            unsigned_slot(seconds(*dt - EPOCH)?, tag, LDT)?
        }
        _ => return Err(format!("its type cannot hold {value:?}")),
    };
    Ok(slot)
}

/// The slot holding the unsigned count of a date or time of day: 64 bits for
/// the long type `long`, otherwise 32 bits held as the VM holds a 32-bit value.
fn unsigned_slot(count: i128, tag: u8, long: u8) -> Result<Slot, String> {
    if tag == long {
        Ok(Slot::from_u64(fit::<u64>(count)?))
    } else {
        Ok(Slot::from_i32(fit::<u32>(count)? as i32))
    }
}

/// `value` as a `T`, or why `T` cannot hold it.
fn fit<T: TryFrom<i128>>(value: i128) -> Result<T, String> {
    T::try_from(value).map_err(|_| format!("{value} is out of range"))
}

/// The whole milliseconds in `duration`, or why it has a finer part.
fn millis(duration: Duration) -> Result<i128, String> {
    let nanos = duration.whole_nanoseconds();
    if nanos % 1_000_000 != 0 {
        return Err(format!("{duration} is finer than a millisecond"));
    }
    Ok(nanos / 1_000_000)
}

/// The whole seconds in `duration`, or why it has a finer part.
fn seconds(duration: Duration) -> Result<i128, String> {
    let nanos = duration.whole_nanoseconds();
    if nanos % 1_000_000_000 != 0 {
        return Err(format!("{duration} is finer than a second"));
    }
    Ok(nanos / 1_000_000_000)
}

const EPOCH: PrimitiveDateTime = datetime!(1970-01-01 0:00);

/// The unsigned count a date or time of day holds: 64 bits for the long type
/// `long`, otherwise the low 32 bits.
fn unsigned(slot: Slot, tag: u8, long: u8) -> i64 {
    if tag == long {
        slot.as_i64()
    } else {
        i64::from(slot.as_i32() as u32)
    }
}

/// The content of the string whose header is at `offset` in `data_region`:
/// `[max_length: u16][cur_length: u16][char_width: u16]` then `cur_length`
/// code units of Latin-1 or UTF-16LE (ADR-0035).
fn read_string(data_region: &[u8], offset: u32) -> String {
    let start = offset as usize;
    let u16_at = |i: usize| u16::from_le_bytes([data_region[i], data_region[i + 1]]);
    let length = usize::from(u16_at(start + 2));
    let data = start + STRING_HEADER_BYTES;
    match u16_at(start + 4) {
        1 => data_region[data..data + length]
            .iter()
            .map(|&b| char::from(b))
            .collect(),
        2 => char::decode_utf16((0..length).map(|i| u16_at(data + 2 * i)))
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect(),
        width => panic!("a string header records char width {width}"),
    }
}
