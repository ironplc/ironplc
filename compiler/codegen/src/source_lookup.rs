//! Bridge from the codegen driver to the source bytes the parser saw.
//!
//! Codegen needs the original source bytes for each [`FileId`] referenced
//! by an AST node so it can hash them with BLAKE3 into the debug
//! section's SOURCE_FILE_TABLE (tag 6). The bytes live in the driver
//! (the CLI, LSP, playground, MCP, etc.) rather than in codegen, so the
//! driver passes a [`SourceLookup`] adapter into [`super::compile`].
//!
//! When the driver doesn't have (or doesn't want to ship) the original
//! bytes — common in unit tests and when compiling synthetic ASTs —
//! it can pass [`EmptyLookup`]. Codegen still registers every seen
//! [`FileId`] in the table, but with an all-zero `content_hash` that a
//! debugger interprets as "drift check unavailable" rather than "hash
//! mismatch" (per the spec entry on `SourceFileEntry`).

use ironplc_dsl::core::FileId;

/// Bridge from a [`FileId`] back to the source bytes the parser saw.
///
/// Implementations should return the **exact** bytes (no normalization,
/// no trailing-newline fixup) so the BLAKE3 hash codegen records
/// matches the hash a debugger computes by reading the file from disk.
///
/// Returning `None` is fine — codegen falls back to an all-zero hash
/// that disables the drift check for that file.
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
