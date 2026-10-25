//! How a backend reads the source bytes the parser saw, to hash them into
//! its debug information and to turn a span into a line and a column
//! (`specs/steering/backend-guide.md`, "Build debug information").
//!
//! The bytes live in the driver (the CLI, LSP, playground, MCP, etc.)
//! rather than in a backend, so the driver passes a [`SourceLookup`] to the
//! backend. A driver that does not have the bytes, or does not want to ship
//! them, passes [`EmptyLookup`].

use ironplc_dsl::core::FileId;

/// Bridge from a [`FileId`] back to the source bytes the parser saw.
///
/// Implementations should return the **exact** bytes (no normalization,
/// no trailing-newline fixup) so the hash a backend records matches the
/// hash a debugger computes by reading the file from disk.
///
/// Returning `None` is fine: the backend then records that it has no bytes
/// for that file.
pub trait SourceLookup {
    fn source_bytes(&self, file_id: &FileId) -> Option<&[u8]>;
}

/// A [`SourceLookup`] that has no source bytes for any file.
///
/// Convenient default for unit tests and synthetic-AST callers that
/// don't want to track source bytes.
pub struct EmptyLookup;

impl SourceLookup for EmptyLookup {
    fn source_bytes(&self, _file_id: &FileId) -> Option<&[u8]> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bytes_when_empty_lookup_then_none() {
        assert!(EmptyLookup
            .source_bytes(&FileId::from_string("main.st"))
            .is_none());
    }
}
