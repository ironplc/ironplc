use core::fmt;

use crate::id_types::ConstantIndex;

/// Errors that can occur when reading or writing a bytecode container.
#[derive(Debug)]
pub enum ContainerError {
    /// An I/O error occurred during reading or writing.
    #[cfg(feature = "std")]
    Io(std::io::Error),
    /// The file does not start with the expected magic number.
    InvalidMagic,
    /// The container format version is not supported.
    UnsupportedVersion,
    /// A constant entry has an unrecognized type tag.
    InvalidConstantType(u8),
    /// A constant pool index is out of bounds.
    InvalidConstantIndex(ConstantIndex),
    /// A section's actual size does not match the declared size.
    SectionSizeMismatch,
    /// A task entry has an unrecognized task type tag.
    InvalidTaskType(u8),
    /// The debug section contains invalid data.
    #[cfg(feature = "std")]
    InvalidDebugSection,
    /// A field entry has an unrecognized field type tag.
    InvalidFieldType(u8),
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            ContainerError::Io(e) => write!(f, "I/O error: {e}"),
            ContainerError::InvalidMagic => write!(f, "invalid magic number"),
            ContainerError::UnsupportedVersion => write!(f, "unsupported container format version"),
            ContainerError::InvalidConstantType(t) => {
                write!(f, "invalid constant type tag: {t}")
            }
            ContainerError::InvalidConstantIndex(idx) => {
                write!(f, "constant pool index out of bounds: {}", idx.raw())
            }
            ContainerError::SectionSizeMismatch => write!(f, "section size mismatch"),
            ContainerError::InvalidTaskType(t) => {
                write!(f, "invalid task type tag: {t}")
            }
            #[cfg(feature = "std")]
            ContainerError::InvalidDebugSection => write!(f, "invalid debug section"),
            ContainerError::InvalidFieldType(t) => {
                write!(f, "invalid field type: {t}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ContainerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ContainerError::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for ContainerError {
    fn from(e: std::io::Error) -> Self {
        ContainerError::Io(e)
    }
}
