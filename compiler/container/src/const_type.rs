use crate::{CharWidth, ContainerError};

/// Type tags for constant pool entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConstType {
    I32 = 0,
    U32 = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
    Str = 6,
    WStr = 7,
}

impl ConstType {
    pub(crate) fn from_u8(v: u8) -> Result<Self, ContainerError> {
        match v {
            0 => Ok(ConstType::I32),
            1 => Ok(ConstType::U32),
            2 => Ok(ConstType::I64),
            3 => Ok(ConstType::U64),
            4 => Ok(ConstType::F32),
            5 => Ok(ConstType::F64),
            6 => Ok(ConstType::Str),
            7 => Ok(ConstType::WStr),
            _ => Err(ContainerError::InvalidConstantType(v)),
        }
    }

    /// Returns the human-readable name for this constant type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConstType::I32 => "I32",
            ConstType::U32 => "U32",
            ConstType::I64 => "I64",
            ConstType::U64 => "U64",
            ConstType::F32 => "F32",
            ConstType::F64 => "F64",
            ConstType::Str => "Str",
            ConstType::WStr => "WStr",
        }
    }

    /// Returns the per-code-unit byte width for string-typed entries, or
    /// `None` for non-string types.
    pub fn char_width(&self) -> Option<CharWidth> {
        match self {
            ConstType::Str => Some(CharWidth::Narrow),
            ConstType::WStr => Some(CharWidth::Wide),
            _ => None,
        }
    }

    /// Returns `true` for string-typed entries ([`ConstType::Str`] and
    /// [`ConstType::WStr`]), whose value lives in `str_value` rather than
    /// inline `primitive` bytes.
    pub fn is_string_like(&self) -> bool {
        matches!(self, ConstType::Str | ConstType::WStr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_type_from_u8_when_valid_tags_then_returns_variant() {
        assert_eq!(ConstType::from_u8(0).unwrap(), ConstType::I32);
        assert_eq!(ConstType::from_u8(1).unwrap(), ConstType::U32);
        assert_eq!(ConstType::from_u8(2).unwrap(), ConstType::I64);
        assert_eq!(ConstType::from_u8(3).unwrap(), ConstType::U64);
        assert_eq!(ConstType::from_u8(4).unwrap(), ConstType::F32);
        assert_eq!(ConstType::from_u8(5).unwrap(), ConstType::F64);
        assert_eq!(ConstType::from_u8(6).unwrap(), ConstType::Str);
        assert_eq!(ConstType::from_u8(7).unwrap(), ConstType::WStr);
    }

    #[test]
    fn const_type_from_u8_when_invalid_tag_then_returns_error() {
        assert!(matches!(
            ConstType::from_u8(99),
            Err(ContainerError::InvalidConstantType(99))
        ));
    }

    #[test]
    fn const_type_as_str_when_each_variant_then_returns_name() {
        assert_eq!(ConstType::I32.as_str(), "I32");
        assert_eq!(ConstType::U32.as_str(), "U32");
        assert_eq!(ConstType::I64.as_str(), "I64");
        assert_eq!(ConstType::U64.as_str(), "U64");
        assert_eq!(ConstType::F32.as_str(), "F32");
        assert_eq!(ConstType::F64.as_str(), "F64");
        assert_eq!(ConstType::Str.as_str(), "Str");
        assert_eq!(ConstType::WStr.as_str(), "WStr");
    }

    #[test]
    fn const_type_char_width_when_string_types_then_matches_encoding() {
        assert_eq!(ConstType::Str.char_width(), Some(CharWidth::Narrow));
        assert_eq!(ConstType::WStr.char_width(), Some(CharWidth::Wide));
        assert_eq!(ConstType::I32.char_width(), None);
        assert_eq!(ConstType::F64.char_width(), None);
    }

    #[test]
    fn const_type_is_string_like_when_string_types_then_true_else_false() {
        assert!(ConstType::Str.is_string_like());
        assert!(ConstType::WStr.is_string_like());
        assert!(!ConstType::I32.is_string_like());
        assert!(!ConstType::U32.is_string_like());
        assert!(!ConstType::I64.is_string_like());
        assert!(!ConstType::U64.is_string_like());
        assert!(!ConstType::F32.is_string_like());
        assert!(!ConstType::F64.is_string_like());
    }
}
