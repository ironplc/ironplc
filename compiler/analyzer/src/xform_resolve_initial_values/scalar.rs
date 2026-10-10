//! The scalar values a declaration can start with: a constant converted to
//! the type of the place it initializes, an enumerated value, and the
//! implicit default of a scalar type.
//!
//! A value is a literal of the kind its type stores (see
//! [`Value::Constant`]): an integer literal that initializes a `REAL` is
//! rewritten as a real literal, and one that initializes a `BOOL` as `TRUE`
//! or `FALSE`, keeping the span the program wrote it at.
//!
//! Each conversion answers `None` for a value the place cannot hold -- a
//! real literal for an `INT`, `300` for a `SINT`, a member of another
//! enumeration. A semantic rule reports every one of those, so the
//! declaration is simply left as written rather than given a wrong value.

use ironplc_dsl::common::{
    Boolean, BooleanLiteral, ConstantKind, EnumeratedValue, Integer, IntegerLiteral, RealLiteral,
    SignedInteger,
};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::time::{
    DateAndTimeLiteral, DateLiteral, DurationLiteral, TemporalWidth, TimeOfDayLiteral,
};

use super::value::Value;
use crate::semantic_type::{ByteSized, SemanticType};
use crate::value_range;

/// The value a scalar of `representation` starts with when nothing
/// declares one: `FALSE`, zero, an enumeration's default member, or a
/// subrange's lower bound, with a synthesized span. `None` for a type that
/// is not a scalar.
pub(super) fn implicit_default(representation: &SemanticType) -> Option<Value> {
    let span = SourceSpan::synthesized();
    let constant = match representation {
        SemanticType::Bool => ConstantKind::Boolean(BooleanLiteral {
            value: Boolean::False,
            span,
        }),
        SemanticType::Int { .. } | SemanticType::UInt { .. } | SemanticType::Bytes { .. } => {
            integer(0, span)
        }
        SemanticType::Subrange { min_value, .. } => integer(*min_value, span),
        SemanticType::Real { .. } => ConstantKind::RealLiteral(RealLiteral {
            value: 0.0,
            data_type: None,
            span,
        }),
        SemanticType::Time { size } => {
            ConstantKind::Duration(DurationLiteral::zero(span).with_width(width(size)))
        }
        SemanticType::TimeOfDay { size } => {
            ConstantKind::TimeOfDay(TimeOfDayLiteral::midnight().with_width(width(size)))
                .with_span(span)
        }
        SemanticType::Date { size } => {
            ConstantKind::Date(DateLiteral::epoch().with_width(width(size))).with_span(span)
        }
        SemanticType::DateAndTime { size } => {
            ConstantKind::DateAndTime(DateAndTimeLiteral::epoch().with_width(width(size)))
                .with_span(span)
        }
        SemanticType::Enumeration { members, .. } => {
            let member = members.default_member()?;
            return Some(Value::Enumerated(EnumeratedValue {
                type_name: None,
                value: member.name.clone().with_position(span),
                explicit_value: None,
            }));
        }
        _ => return None,
    };
    Some(Value::Constant(constant))
}

/// `constant` as a value of `representation`, the type of the place it
/// initializes.
pub(super) fn from_constant(
    constant: &ConstantKind,
    representation: &SemanticType,
) -> Option<Value> {
    convert(constant, representation).map(Value::Constant)
}

/// `value` as a value of the enumeration `representation`, when it is one
/// of its members.
pub(super) fn from_enumerated_value(
    value: &EnumeratedValue,
    representation: &SemanticType,
) -> Option<Value> {
    representation
        .enumeration_members()?
        .contains(&value.value)
        .then(|| Value::Enumerated(value.clone()))
}

/// Converts `constant` to the literal `representation` stores it as.
fn convert(constant: &ConstantKind, representation: &SemanticType) -> Option<ConstantKind> {
    match representation {
        SemanticType::Bool => match constant {
            ConstantKind::Boolean(_) => Some(constant.clone()),
            // `b : BOOL := 1` where an integer initializer for a BOOL is
            // allowed; anywhere else a rule reports it.
            ConstantKind::IntegerLiteral(literal) => {
                let value = match i128::try_from(literal.value.clone()) {
                    Ok(0) => Boolean::False,
                    Ok(1) => Boolean::True,
                    _ => return None,
                };
                Some(ConstantKind::Boolean(BooleanLiteral {
                    value,
                    span: constant.span(),
                }))
            }
            _ => None,
        },
        SemanticType::Int { .. }
        | SemanticType::UInt { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Subrange { .. } => {
            let value = match constant {
                ConstantKind::IntegerLiteral(literal) => {
                    i128::try_from(literal.value.clone()).ok()?
                }
                ConstantKind::BitStringLiteral(literal) => {
                    i128::try_from(literal.value.value).ok()?
                }
                _ => return None,
            };
            let (minimum, maximum) = value_range::of(representation)?;
            (minimum..=maximum)
                .contains(&value)
                .then(|| constant.clone())
        }
        // An integer converts straight to the width it is stored at, so
        // that a `REAL` is rounded once rather than through an `LREAL`.
        SemanticType::Real { size } => match constant {
            ConstantKind::RealLiteral(_) => Some(constant.clone()),
            ConstantKind::IntegerLiteral(literal) => {
                let integer = i128::try_from(literal.value.clone()).ok()?;
                let value = match size {
                    ByteSized::B64 => integer as f64,
                    _ => f64::from(integer as f32),
                };
                Some(ConstantKind::RealLiteral(RealLiteral {
                    value,
                    data_type: None,
                    span: constant.span(),
                }))
            }
            _ => None,
        },
        // A duration can be negative (ADR-0021); the calendar types count
        // up from their epoch (ADR-0025).
        SemanticType::Time { size } => match constant {
            ConstantKind::Duration(literal) => {
                counted(literal.interval.whole_milliseconds(), size, true)
            }
            _ => false,
        }
        .then(|| constant.clone()),
        SemanticType::TimeOfDay { size } => match constant {
            ConstantKind::TimeOfDay(literal) => {
                counted(i128::from(literal.whole_milliseconds()), size, false)
            }
            _ => false,
        }
        .then(|| constant.clone()),
        SemanticType::Date { size } => match constant {
            ConstantKind::Date(literal) => {
                counted(i128::from(literal.seconds_since_epoch()), size, false)
            }
            _ => false,
        }
        .then(|| constant.clone()),
        SemanticType::DateAndTime { size } => match constant {
            ConstantKind::DateAndTime(literal) => {
                counted(i128::from(literal.seconds_since_epoch()), size, false)
            }
            _ => false,
        }
        .then(|| constant.clone()),
        _ => None,
    }
}

/// Whether a time-like type of `size` holds `count`.
fn counted(count: i128, size: &ByteSized, signed: bool) -> bool {
    let bits = u32::from(size.as_bytes()) * 8;
    value_range::fits(count, bits, signed)
}

/// The member of a temporal family a type of `size` is.
fn width(size: &ByteSized) -> TemporalWidth {
    match size {
        ByteSized::B64 => TemporalWidth::Long,
        _ => TemporalWidth::Short,
    }
}

/// The integer literal `value`, at `span`.
fn integer(value: i128, span: SourceSpan) -> ConstantKind {
    ConstantKind::IntegerLiteral(IntegerLiteral {
        value: SignedInteger {
            value: Integer {
                span,
                value: value.unsigned_abs(),
            },
            is_neg: value < 0,
        },
        data_type: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::Located;

    fn integer_literal(text: &str) -> ConstantKind {
        ConstantKind::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new(text, SourceSpan::range(3, 4)).unwrap(),
            data_type: None,
        })
    }

    #[test]
    fn from_constant_when_integer_for_real_then_real_literal_at_same_span() {
        let value = from_constant(
            &integer_literal("3"),
            &SemanticType::Real {
                size: ByteSized::B32,
            },
        );

        let Some(Value::Constant(ConstantKind::RealLiteral(literal))) = value else {
            panic!("expected a real literal, got {value:?}");
        };
        assert_eq!(literal.value, 3.0);
        assert_eq!(literal.span.start, 3);
    }

    #[test]
    fn from_constant_when_value_outside_type_then_none() {
        let value = from_constant(
            &integer_literal("300"),
            &SemanticType::Int {
                size: ByteSized::B8,
            },
        );

        assert!(value.is_none());
    }

    #[test]
    fn from_constant_when_real_for_integer_then_none() {
        let constant = ConstantKind::RealLiteral(RealLiteral {
            value: 2.5,
            data_type: None,
            span: SourceSpan::default(),
        });

        let value = from_constant(
            &constant,
            &SemanticType::Int {
                size: ByteSized::B16,
            },
        );

        assert!(value.is_none());
    }

    #[test]
    fn implicit_default_when_subrange_then_synthesized_lower_bound() {
        let subrange = SemanticType::Subrange {
            base_type: Box::new(SemanticType::Int {
                size: ByteSized::B32,
            }),
            min_value: -10,
            max_value: 100,
        };

        let value = implicit_default(&subrange);

        let Some(Value::Constant(constant @ ConstantKind::IntegerLiteral(literal))) = &value else {
            panic!("expected an integer literal, got {value:?}");
        };
        assert_eq!(i128::try_from(literal.value.clone()).ok(), Some(-10));
        assert!(constant.span().is_synthesized());
    }
}
