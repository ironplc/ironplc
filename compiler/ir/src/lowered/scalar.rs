//! The operation types of scalar values (ADR-0001).

use std::fmt;

/// Whether an integer operation treats its operands as signed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signedness {
    Signed,
    Unsigned,
}

/// The type a scalar value is operated at: one of the four operation widths
/// of ADR-0001, with a signedness for an integer, or a reference or interface
/// value, which has no width in the lowered program.
///
/// A `ScalarType` does not say whether a value is a `BOOL`: a `BOOL` is
/// operated at [`ScalarType::BOOL`], the operation type of a `DINT`. Lowering
/// chooses the logical or the bitwise form of an operator from the source
/// type (REQ-LOW-lowering-048).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarType {
    I32(Signedness),
    I64(Signedness),
    F32,
    F64,
    Ref,
    Interface,
}

impl ScalarType {
    /// The operation type of a `BOOL`, and of the result of a comparison.
    pub const BOOL: ScalarType = ScalarType::I32(Signedness::Signed);

    /// The width of a numeric operation type in bits; `None` for a reference
    /// or an interface value.
    pub fn bits(self) -> Option<u8> {
        match self {
            ScalarType::I32(_) | ScalarType::F32 => Some(32),
            ScalarType::I64(_) | ScalarType::F64 => Some(64),
            ScalarType::Ref | ScalarType::Interface => None,
        }
    }

    /// The signedness of an integer operation type; `None` for any other.
    pub fn signedness(self) -> Option<Signedness> {
        match self {
            ScalarType::I32(signedness) | ScalarType::I64(signedness) => Some(signedness),
            ScalarType::F32 | ScalarType::F64 | ScalarType::Ref | ScalarType::Interface => None,
        }
    }

    /// Whether the type is an integer operation type.
    pub fn is_integer(self) -> bool {
        self.signedness().is_some()
    }

    /// Whether the type is a real operation type.
    pub fn is_real(self) -> bool {
        matches!(self, ScalarType::F32 | ScalarType::F64)
    }

    /// Whether arithmetic applies to the type: an integer or a real.
    pub fn is_numeric(self) -> bool {
        self.is_integer() || self.is_real()
    }

    /// The smallest and largest integer a value of an integer operation type
    /// holds; `None` for any other.
    pub fn integer_range(self) -> Option<(i128, i128)> {
        match self {
            ScalarType::I32(Signedness::Signed) => Some((i32::MIN.into(), i32::MAX.into())),
            ScalarType::I32(Signedness::Unsigned) => Some((0, u32::MAX.into())),
            ScalarType::I64(Signedness::Signed) => Some((i64::MIN.into(), i64::MAX.into())),
            ScalarType::I64(Signedness::Unsigned) => Some((0, u64::MAX.into())),
            ScalarType::F32 | ScalarType::F64 | ScalarType::Ref | ScalarType::Interface => None,
        }
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ScalarType::I32(Signedness::Signed) => "i32s",
            ScalarType::I32(Signedness::Unsigned) => "i32u",
            ScalarType::I64(Signedness::Signed) => "i64s",
            ScalarType::I64(Signedness::Unsigned) => "i64u",
            ScalarType::F32 => "f32",
            ScalarType::F64 => "f64",
            ScalarType::Ref => "ref",
            ScalarType::Interface => "interface",
        };
        f.write_str(text)
    }
}
