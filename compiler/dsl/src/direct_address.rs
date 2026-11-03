//! Direct addresses: the `%IX0.0`-style locations of directly represented
//! variables, and the location and size prefixes that make them up.
//!
//! See section 2.4.1.1.

use lazy_static::lazy_static;
use regex::Regex;
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

lazy_static! {
    // Accept what the `DirectAddress` and `DirectAddressIncomplete` tokens
    // accept, matched against upper-cased text: `[0-9]` rather than the
    // Unicode-aware `\d`, and fields of any number of digits with single
    // `_` separators.
    static ref DIRECT_ADDRESS_UNASSIGNED: Regex = Regex::new(r"^%([IQM])\*$").unwrap();
    static ref DIRECT_ADDRESS: Regex =
        Regex::new(r"^%([IQM])([XBWDL])?([0-9](_?[0-9])*(\.[0-9](_?[0-9])*)*)$").unwrap();
}

impl TryFrom<&str> for AddressAssignment {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // The letters of a direct address are not case-sensitive (`%ix0.0`).
        let value = value.to_ascii_uppercase();

        if let Some(cap) = DIRECT_ADDRESS_UNASSIGNED.captures(&value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            return Ok(AddressAssignment {
                location: location_prefix,
                size: SizePrefix::Unspecified,
                address: vec![],
                position: SourceSpan::default(),
            });
        }

        if let Some(cap) = DIRECT_ADDRESS.captures(&value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            // The size letter is optional (`%I1`); without it the size is `Nil`.
            let size_prefix = SizePrefix::try_from(cap.get(2).map_or("", |m| m.as_str()))?;
            let pos = cap[3]
                .split('.')
                .map(|v| v.replace('_', "").parse::<u32>())
                .collect::<Result<Vec<u32>, _>>()
                .map_err(|_| "Direct address field does not fit in 32 bits")?;

            return Ok(AddressAssignment {
                location: location_prefix,
                size: size_prefix,
                address: pos,
                position: SourceSpan::default(),
            });
        }

        Err("Value not convertible to direct variable")
    }
}

impl fmt::Debug for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

/// The address as IEC 61131-3 writes it: `%IX0.0`, `%MW10`, `%Q1.2.3`, or
/// `%I*` for an address assigned later in a `VAR_CONFIG`.
impl fmt::Display for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = match self.location {
            LocationPrefix::I => "I",
            LocationPrefix::Q => "Q",
            LocationPrefix::M => "M",
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
        for (i, field) in self.address.iter().enumerate() {
            if i > 0 {
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

    #[test]
    fn try_from_when_no_size_prefix_then_size_is_nil() {
        let address = AddressAssignment::try_from("%I1").unwrap();
        assert_eq!(address.location, LocationPrefix::I);
        assert_eq!(address.size, SizePrefix::Nil);
        assert_eq!(address.address, vec![1]);
    }

    #[test]
    fn try_from_when_non_ascii_digit_then_err() {
        assert!(AddressAssignment::try_from("%IX\u{0663}").is_err());
    }
}
