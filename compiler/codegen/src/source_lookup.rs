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

use std::collections::HashMap;

use ironplc_container::{SourceFileEntry, SourceFileId, SOURCE_FILE_HASH_LEN};
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

/// Accumulator for the debug section's `SOURCE_FILE_TABLE` (tag 6).
///
/// `compile()` walks the library at the start of code generation,
/// calling [`Self::register`] for every [`FileId`] referenced by a
/// top-level POU. The codegen statement emitter then reads back two
/// things per statement:
///
/// 1. The [`SourceFileId`] index, via [`Self::get`], used as
///    `LineMapEntry.file_id`.
/// 2. The cached source bytes, via [`Self::source_bytes`], used to
///    convert a `SourceSpan.start` byte offset to a 1-based
///    `(line, column)` for [`ironplc_container::LineMapEntry`].
///
/// `compile()` hands [`Self::into_entries`] to
/// `ContainerBuilder::add_source_files` at the end of compilation.
#[derive(Default)]
pub(crate) struct SourceFileRegistry {
    /// Index assigned to each known `FileId`. Insertion order is the
    /// resulting `SourceFileId` value.
    ids: HashMap<FileId, SourceFileId>,
    /// Owned copy of each registered file's source bytes, used by the
    /// statement emitter for (line, column) conversion. Files
    /// registered without bytes (the [`EmptyLookup`] case) have no
    /// entry here, so the emitter skips line-map recording for them.
    bytes: HashMap<FileId, Vec<u8>>,
    /// Entries to feed to `ContainerBuilder::add_source_files` at the
    /// end of `compile()`. Index N is the entry for
    /// `SourceFileId::new(N)`.
    entries: Vec<SourceFileEntry>,
}

impl SourceFileRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Registers a `FileId` (idempotent). Returns its `SourceFileId`.
    ///
    /// `bytes` is the source content the parser saw, or `None` when
    /// the caller doesn't have it. The first form records a BLAKE3
    /// hash; the second records an all-zero hash (per the spec,
    /// "drift check unavailable") and disables line-map recording
    /// for this file.
    pub(crate) fn register(&mut self, file_id: &FileId, bytes: Option<&[u8]>) -> SourceFileId {
        if let Some(&id) = self.ids.get(file_id) {
            return id;
        }
        let content_hash: [u8; SOURCE_FILE_HASH_LEN] = match bytes {
            Some(b) => *blake3::hash(b).as_bytes(),
            None => [0u8; SOURCE_FILE_HASH_LEN],
        };
        let id = SourceFileId::new(self.entries.len() as u16);
        self.entries.push(SourceFileEntry {
            path: file_id.to_string(),
            content_hash,
        });
        self.ids.insert(file_id.clone(), id);
        if let Some(b) = bytes {
            self.bytes.insert(file_id.clone(), b.to_vec());
        }
        id
    }

    /// Returns the previously-registered `SourceFileId` for `file_id`,
    /// or `None` if `register` was never called for it.
    pub(crate) fn get(&self, file_id: &FileId) -> Option<SourceFileId> {
        self.ids.get(file_id).copied()
    }

    /// Returns the cached source bytes for `file_id`, or `None` if the
    /// file was registered without bytes (or not at all).
    pub(crate) fn source_bytes(&self, file_id: &FileId) -> Option<&[u8]> {
        self.bytes.get(file_id).map(Vec::as_slice)
    }

    /// Consumes the registry and returns the entries to feed to
    /// `ContainerBuilder::add_source_files`.
    pub(crate) fn into_entries(self) -> Vec<SourceFileEntry> {
        self.entries
    }
}
