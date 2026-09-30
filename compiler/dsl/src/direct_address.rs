//! Directly represented variables: the location prefix, size prefix and
//! hierarchical address of a direct address such as `%IX0.0`.
//!
//! See section 2.4.1.1 and 2.4.3.1.

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
    static ref DIRECT_ADDRESS_UNASSIGNED: Regex = Regex::new(r"%([IQM])\*").unwrap();
    static ref DIRECT_ADDRESS: Regex = Regex::new(r"%([IQM])([XBWDL])?(\d(\.\d)*)").unwrap();
}

impl TryFrom<&str> for AddressAssignment {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(cap) = DIRECT_ADDRESS_UNASSIGNED.captures(value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            return Ok(AddressAssignment {
                location: location_prefix,
                size: SizePrefix::Unspecified,
                address: vec![],
                position: SourceSpan::default(),
            });
        }

        if let Some(cap) = DIRECT_ADDRESS.captures(value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            let size_prefix = SizePrefix::try_from(&cap[2])?;
            let pos: Vec<u32> = cap[3]
                .split('.')
                .map(|v| v.parse::<u32>().unwrap())
                .collect();

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
