//! Directly represented variables: the location prefix, size prefix and
//! hierarchical address of a direct address such as `%IX0.0`.
//!
//! See section 2.4.1.1 and 2.4.3.1.

use std::fmt;

use dsl_macro_derive::Recurse;

use crate::core::SourceSpan;
use crate::fold::Fold;
use crate::visitor::Visitor;

/// Location prefix for directly represented variables.
///
/// See section 2.4.1.1.
#[derive(Clone, Debug, PartialEq)]
pub enum LocationPrefix {
    /// Input location
    I,
    /// Output location
    Q,
    /// Memory location
    M,
}

impl TryFrom<Option<char>> for LocationPrefix {
    type Error = &'static str;

    fn try_from(value: Option<char>) -> Result<Self, Self::Error> {
        match value {
            Some('I') => Ok(LocationPrefix::I),
            Some('Q') => Ok(LocationPrefix::Q),
            Some('M') => Ok(LocationPrefix::M),
            _ => Err("Value must be one of I, Q, M"),
        }
    }
}

impl TryFrom<&str> for LocationPrefix {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let c = value.chars().nth(0);
        LocationPrefix::try_from(c)
    }
}

/// Size prefix for directly represented variables. Defines how many bits
/// are associated with the variable.
///
/// See section 2.4.1.1.
#[derive(Clone, Debug, PartialEq)]
pub enum SizePrefix {
    /// Unspecified (indicated by asterisk)
    Unspecified,
    /// Single bit size
    Nil,
    /// Single bit size
    X,
    /// 8-bit size
    B,
    /// 16-bit size
    W,
    /// 32-bit size
    D,
    /// 64-bit size
    L,
}

impl TryFrom<Option<char>> for SizePrefix {
    type Error = &'static str;

    fn try_from(value: Option<char>) -> Result<Self, Self::Error> {
        match value {
            Some('*') => Ok(SizePrefix::Unspecified),
            Some('X') => Ok(SizePrefix::X),
            Some('B') => Ok(SizePrefix::B),
            Some('W') => Ok(SizePrefix::W),
            Some('D') => Ok(SizePrefix::D),
            Some('L') => Ok(SizePrefix::L),
            None => Ok(SizePrefix::Nil),
            _ => Err("Value must be one of *, X, B, W, D, L, NIL"),
        }
    }
}

impl TryFrom<&str> for SizePrefix {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let c = value.chars().nth(0);
        SizePrefix::try_from(c)
    }
}

/// Location assignment for a variable.
///
/// See section 2.4.3.1.
#[derive(Clone, PartialEq, Recurse)]
pub struct AddressAssignment {
    #[recurse(ignore)]
    pub location: LocationPrefix,
    #[recurse(ignore)]
    pub size: SizePrefix,
    #[recurse(ignore)]
    pub address: Vec<u32>,
    pub position: SourceSpan,
}

impl AddressAssignment {
    /// Returns this address with `position` as its source position.
    ///
    /// [`AddressAssignment::try_from`] parses the address out of the token
    /// text alone, which carries no position, so the caller puts the token's
    /// span back.
    pub fn with_position(mut self, position: SourceSpan) -> Self {
        self.position = position;
        self
    }
}

/// The error for text that is not a direct address.
const NOT_A_DIRECT_ADDRESS: &str = "Value not convertible to direct variable";

/// Parses a direct address following IEC 61131-3 (B.1.4.1):
///
/// ```text
/// direct_variable ::= '%' location_prefix size_prefix integer {'.' integer}
/// integer         ::= digit {['_'] digit}
/// incompl_location ::= '%' ('I' | 'Q' | 'M') '*'
/// ```
///
/// The size prefix is optional and letters are case-insensitive, as the
/// lexer accepts them. A field that does not fit in a `u32` is an error.
impl TryFrom<&str> for AddressAssignment {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let rest = value.strip_prefix('%').ok_or(NOT_A_DIRECT_ADDRESS)?;
        let mut chars = rest.chars();
        let location = LocationPrefix::try_from(chars.next().map(|c| c.to_ascii_uppercase()))?;
        let rest = chars.as_str();

        if rest == "*" {
            return Ok(AddressAssignment {
                location,
                size: SizePrefix::Unspecified,
                address: vec![],
                position: SourceSpan::default(),
            });
        }

        let (size, fields) = match rest.chars().next() {
            Some(c) if c.is_ascii_alphabetic() => (
                SizePrefix::try_from(Some(c.to_ascii_uppercase()))?,
                &rest[c.len_utf8()..],
            ),
            _ => (SizePrefix::Nil, rest),
        };

        let address = fields
            .split('.')
            .map(parse_field)
            .collect::<Result<Vec<u32>, _>>()?;

        Ok(AddressAssignment {
            location,
            size,
            address,
            position: SourceSpan::default(),
        })
    }
}

/// Parses one field of a direct address: `digit {['_'] digit}`.
fn parse_field(field: &str) -> Result<u32, &'static str> {
    let well_formed = field.starts_with(|c: char| c.is_ascii_digit())
        && field.ends_with(|c: char| c.is_ascii_digit())
        && !field.contains("__")
        && field.chars().all(|c| c.is_ascii_digit() || c == '_');
    if !well_formed {
        return Err(NOT_A_DIRECT_ADDRESS);
    }
    field
        .replace('_', "")
        .parse::<u32>()
        .map_err(|_| "Direct address field does not fit in 32 bits")
}

impl fmt::Debug for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

/// Spells the address as IEC 61131-3 source text, such as `%IX0.0`,
/// `%MW10` or the incomplete `%I*`.
impl fmt::Display for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = match self.location {
            LocationPrefix::I => 'I',
            LocationPrefix::Q => 'Q',
            LocationPrefix::M => 'M',
        };
        let size = match self.size {
            SizePrefix::Unspecified => "*",
            SizePrefix::Nil => "",
            SizePrefix::X => "X",
            SizePrefix::B => "B",
            SizePrefix::W => "W",
            SizePrefix::D => "D",
            SizePrefix::L => "L",
        };
        write!(f, "%{location}{size}")?;
        for (index, field) in self.address.iter().enumerate() {
            if index > 0 {
                f.write_str(".")?;
            }
            write!(f, "{field}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn address(location: LocationPrefix, size: SizePrefix, fields: &[u32]) -> AddressAssignment {
        AddressAssignment {
            location,
            size,
            address: fields.to_vec(),
            position: SourceSpan::default(),
        }
    }

    #[rstest]
    #[case("%MW10", LocationPrefix::M, SizePrefix::W, &[10])]
    #[case("%IX12.7", LocationPrefix::I, SizePrefix::X, &[12, 7])]
    #[case("%QD100", LocationPrefix::Q, SizePrefix::D, &[100])]
    #[case("%IX1.2.3.4", LocationPrefix::I, SizePrefix::X, &[1, 2, 3, 4])]
    #[case("%QL4294967295", LocationPrefix::Q, SizePrefix::L, &[u32::MAX])]
    #[case("%I0", LocationPrefix::I, SizePrefix::Nil, &[0])]
    #[case("%Q10.2", LocationPrefix::Q, SizePrefix::Nil, &[10, 2])]
    #[case("%mb3", LocationPrefix::M, SizePrefix::B, &[3])]
    #[case("%MW1_000", LocationPrefix::M, SizePrefix::W, &[1000])]
    #[case("%I*", LocationPrefix::I, SizePrefix::Unspecified, &[])]
    #[case("%m*", LocationPrefix::M, SizePrefix::Unspecified, &[])]
    fn try_from_when_valid_address_then_fields(
        #[case] text: &str,
        #[case] location: LocationPrefix,
        #[case] size: SizePrefix,
        #[case] fields: &[u32],
    ) {
        assert_eq!(
            AddressAssignment::try_from(text),
            Ok(address(location, size, fields))
        );
    }

    #[rstest]
    #[case::no_percent("MW10")]
    #[case::no_location("%W10")]
    #[case::no_field("%MW")]
    #[case::empty_field("%IX1..2")]
    #[case::trailing_period("%IX1.")]
    #[case::leading_underscore("%MW_1")]
    #[case::trailing_underscore("%MW1_")]
    #[case::double_underscore("%MW1__0")]
    #[case::overflow("%MW4294967296")]
    #[case::size_after_star("%IX*")]
    fn try_from_when_invalid_address_then_error(#[case] text: &str) {
        assert!(AddressAssignment::try_from(text).is_err());
    }

    #[rstest]
    #[case(address(LocationPrefix::I, SizePrefix::X, &[0, 0]), "%IX0.0")]
    #[case(address(LocationPrefix::M, SizePrefix::W, &[10]), "%MW10")]
    #[case(address(LocationPrefix::Q, SizePrefix::Nil, &[1, 2, 3]), "%Q1.2.3")]
    #[case(address(LocationPrefix::I, SizePrefix::Unspecified, &[]), "%I*")]
    fn display_when_address_then_iec_spelling(
        #[case] address: AddressAssignment,
        #[case] expected: &str,
    ) {
        assert_eq!(address.to_string(), expected);
    }
}
