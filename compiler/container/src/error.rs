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
    /// A `char_width` byte (string header, constant-pool tag, or bytecode
    /// operand) is not a recognized [`crate::CharWidth`] discriminant
    /// (1 = `Narrow`, 2 = `Wide`).
    InvalidCharWidth(u8),
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
            ContainerError::InvalidCharWidth(t) => {
                write!(f, "invalid char_width: {t}")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    use std::string::ToString;

    #[test]
    fn container_error_display_when_invalid_magic_then_mentions_magic() {
        let msg = ContainerError::InvalidMagic.to_string();
        assert!(msg.contains("magic"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_unsupported_version_then_mentions_version() {
        let msg = ContainerError::UnsupportedVersion.to_string();
        assert!(msg.contains("version"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_constant_type_then_contains_tag() {
        let msg = ContainerError::InvalidConstantType(42).to_string();
        assert!(msg.contains("42"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_constant_index_then_contains_index() {
        let msg = ContainerError::InvalidConstantIndex(ConstantIndex::new(999)).to_string();
        assert!(msg.contains("999"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_section_size_mismatch_then_mentions_section() {
        let msg = ContainerError::SectionSizeMismatch.to_string();
        assert!(msg.contains("section"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_task_type_then_contains_tag() {
        let msg = ContainerError::InvalidTaskType(7).to_string();
        assert!(msg.contains('7'), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_field_type_then_contains_tag() {
        let msg = ContainerError::InvalidFieldType(5).to_string();
        assert!(msg.contains('5'), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_char_width_then_contains_tag() {
        let msg = ContainerError::InvalidCharWidth(99).to_string();
        assert!(msg.contains("99"), "got: {msg}");
        assert!(msg.contains("char_width"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_io_error_then_mentions_io() {
        let err = ContainerError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        let msg = err.to_string();
        assert!(msg.contains("I/O"), "got: {msg}");
    }

    #[test]
    fn container_error_display_when_invalid_debug_section_then_mentions_debug() {
        let msg = ContainerError::InvalidDebugSection.to_string();
        assert!(msg.contains("debug"), "got: {msg}");
    }

    #[test]
    fn container_error_source_when_io_then_returns_some() {
        let err = ContainerError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        assert!(err.source().is_some());
    }

    #[test]
    fn container_error_source_when_non_io_then_returns_none() {
        assert!(ContainerError::InvalidMagic.source().is_none());
        assert!(ContainerError::UnsupportedVersion.source().is_none());
        assert!(ContainerError::SectionSizeMismatch.source().is_none());
    }

    #[test]
    fn container_error_from_io_error_when_converted_then_io_variant() {
        let io_err = std::io::Error::from(std::io::ErrorKind::UnexpectedEof);
        let err: ContainerError = io_err.into();
        assert!(matches!(err, ContainerError::Io(_)));
    }
}
