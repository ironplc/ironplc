//! Tests that a constant displays as its IEC 61131-3 source spelling.
//!
//! `Display` on each literal is the one place its spelling lives: `plc2plc`
//! writes source through it and tools such as the MCP `pou_scope` show it, so
//! none of these may fall back to `Debug` text.

use time::{Date, Duration, Month, PrimitiveDateTime, Time};

use crate::common::{
    BitStringLiteral, BitStringTypeName, Boolean, BooleanLiteral, CharacterStringLiteral,
    ConstantKind, Integer, IntegerLiteral, IntegerTypeName, RealLiteral, RealTypeName,
    SignedInteger,
};
use crate::core::{Id, SourceSpan};
use crate::sfc::{ActionQualifier, ActionTimeKind};
use crate::time::{
    DateAndTimeLiteral, DateLiteral, DurationLiteral, TemporalWidth, TimeOfDayLiteral,
};

fn integer(value: u128) -> Integer {
    Integer {
        span: SourceSpan::default(),
        value,
    }
}

fn integer_literal(value: u128, is_neg: bool, data_type: Option<IntegerTypeName>) -> ConstantKind {
    ConstantKind::IntegerLiteral(IntegerLiteral {
        value: SignedInteger {
            value: integer(value),
            is_neg,
        },
        data_type,
    })
}

fn real(value: f64, data_type: Option<RealTypeName>) -> ConstantKind {
    ConstantKind::RealLiteral(RealLiteral {
        value,
        data_type,
        span: SourceSpan::default(),
    })
}

fn duration(ms: i64, width: TemporalWidth) -> ConstantKind {
    ConstantKind::Duration(DurationLiteral {
        width,
        ..DurationLiteral::new(SourceSpan::default(), Duration::milliseconds(ms))
    })
}

fn time_of_day(width: TemporalWidth) -> ConstantKind {
    let value = Time::from_hms_milli(10, 0, 0, 250).unwrap();
    let mut lit = TimeOfDayLiteral::new(value);
    lit.width = width;
    ConstantKind::TimeOfDay(lit)
}

fn date(year: i32, width: TemporalWidth) -> ConstantKind {
    let value = Date::from_calendar_date(year, Month::January, 5).unwrap();
    let mut lit = DateLiteral::new(value);
    lit.width = width;
    ConstantKind::Date(lit)
}

fn date_and_time(width: TemporalWidth) -> ConstantKind {
    let value = PrimitiveDateTime::new(
        Date::from_calendar_date(2024, Month::January, 15).unwrap(),
        Time::from_hms(14, 30, 0).unwrap(),
    );
    let mut lit = DateAndTimeLiteral::new(value);
    lit.width = width;
    ConstantKind::DateAndTime(lit)
}

fn bit_string(value: u128, data_type: Option<BitStringTypeName>) -> ConstantKind {
    ConstantKind::BitStringLiteral(BitStringLiteral {
        value: integer(value),
        data_type,
    })
}

#[test]
fn display_when_integer_then_value_with_type_prefix() {
    assert_eq!(integer_literal(5, false, None).to_string(), "5");
    assert_eq!(integer_literal(5, true, None).to_string(), "-5");
    assert_eq!(
        integer_literal(5, false, Some(IntegerTypeName::INT)).to_string(),
        "INT#5"
    );
}

#[test]
fn display_when_whole_real_then_has_decimal_point() {
    assert_eq!(real(2.0, None).to_string(), "2.0");
    assert_eq!(real(2.0, Some(RealTypeName::REAL)).to_string(), "REAL#2.0");
}

#[test]
fn display_when_fractional_real_then_value_as_written() {
    assert_eq!(real(2.5, None).to_string(), "2.5");
    assert_eq!(
        real(2.5, Some(RealTypeName::LREAL)).to_string(),
        "LREAL#2.5"
    );
}

#[test]
fn display_when_boolean_then_bare_keyword() {
    assert_eq!(
        ConstantKind::Boolean(BooleanLiteral::new(Boolean::True)).to_string(),
        "TRUE"
    );
    assert_eq!(
        ConstantKind::Boolean(BooleanLiteral::new(Boolean::False)).to_string(),
        "FALSE"
    );
}

#[test]
fn display_when_string_has_special_characters_then_escaped() {
    let value: Vec<char> = "a'b\"c$\n".chars().collect();
    assert_eq!(
        ConstantKind::CharacterString(CharacterStringLiteral::new(value.clone())).to_string(),
        "'a$'b\"c$$$L'"
    );
    assert_eq!(
        ConstantKind::CharacterString(CharacterStringLiteral::new_wide(value)).to_string(),
        "\"a'b$\"c$$$L\""
    );
}

#[test]
fn display_when_duration_then_prefix_of_its_width_in_milliseconds() {
    assert_eq!(
        duration(1000, TemporalWidth::Short).to_string(),
        "TIME#1000ms"
    );
    assert_eq!(
        duration(1000, TemporalWidth::Long).to_string(),
        "LTIME#1000ms"
    );
}

#[test]
fn display_when_time_of_day_then_prefix_of_its_width_with_fraction() {
    assert_eq!(
        time_of_day(TemporalWidth::Short).to_string(),
        "TIME_OF_DAY#10:00:00.25"
    );
    assert_eq!(
        time_of_day(TemporalWidth::Long).to_string(),
        "LTIME_OF_DAY#10:00:00.25"
    );
}

#[test]
fn display_when_date_then_prefix_of_its_width_and_padded_year() {
    assert_eq!(
        date(2024, TemporalWidth::Short).to_string(),
        "DATE#2024-01-05"
    );
    assert_eq!(
        date(999, TemporalWidth::Long).to_string(),
        "LDATE#0999-01-05"
    );
}

#[test]
fn display_when_date_and_time_then_prefix_of_its_width() {
    assert_eq!(
        date_and_time(TemporalWidth::Short).to_string(),
        "DATE_AND_TIME#2024-01-15-14:30:00"
    );
    assert_eq!(
        date_and_time(TemporalWidth::Long).to_string(),
        "LDATE_AND_TIME#2024-01-15-14:30:00"
    );
}

#[test]
fn display_when_bit_string_then_decimal_value_with_type_prefix() {
    assert_eq!(
        bit_string(255, Some(BitStringTypeName::BYTE)).to_string(),
        "BYTE#255"
    );
    assert_eq!(bit_string(255, None).to_string(), "255");
}

#[test]
fn display_when_untimed_action_qualifier_then_keyword() {
    assert_eq!(ActionQualifier::N.to_string(), "N");
    assert_eq!(ActionQualifier::P.to_string(), "P");
}

#[test]
fn display_when_timed_action_qualifier_then_keyword_and_action_time() {
    let time = ActionTimeKind::Duration(DurationLiteral::new(
        SourceSpan::default(),
        Duration::seconds(1),
    ));
    assert_eq!(ActionQualifier::SD(time).to_string(), "SD, TIME#1000ms");
    assert_eq!(
        ActionQualifier::PR(ActionTimeKind::VariableName(Id::from("t_var"))).to_string(),
        "P1, t_var"
    );
    assert_eq!(
        ActionQualifier::PF(ActionTimeKind::VariableName(Id::from("t_var"))).to_string(),
        "P0, t_var"
    );
}
