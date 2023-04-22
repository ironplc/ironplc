//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use ironplc_dsl::{core::FileId, diagnostic::Diagnostic};

use crate::stages::analyze;

/// A project consisting of one or more files.
///
/// The project acts is akin to an interface for interacting with the compiler
/// for one or more files.
pub trait Project {
    /// Notifies that the file contents changed.
    fn on_did_change_text_document(&self, file_id: &FileId, content: &str) -> Option<Diagnostic> {
        analyze(content, file_id).err()
    }
}
