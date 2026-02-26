use crate::ContainerError;

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
        }
    }
}
