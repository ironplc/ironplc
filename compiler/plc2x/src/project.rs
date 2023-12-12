//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use ironplc_dsl::{core::FileId, diagnostic::Diagnostic, common::Library};

use crate::stages::{analyze, CompilationSet, CompilationSource};

/// A project consisting of one or more files.
///
/// The project acts is akin to an interface for interacting with the compiler
/// for one or more files.
pub trait Project {
    /// Notifies that the file contents changed.
    fn on_did_change_text_document(&mut self, file_id: &FileId, content: &str) -> Option<Diagnostic>;

    /// Parsed libraries that constitute the project.
    fn compilation_set(&self) -> CompilationSet;
}

/// A project is a collection of files used together as a single unit.
pub struct FileBackedProject {
    /// Files that are part of the project but indirectly references
    /// (the standard library).
    external_sources: Vec<(FileId, Library)>,

    /// The user-supplied source files for the project
    sources: Vec<(FileId, String)>,
}

impl FileBackedProject {
    pub fn new() -> Self { 
        FileBackedProject {
            external_sources: Vec::new(),
            sources: Vec::new(),
        }
    }
}

impl Project for FileBackedProject {
    fn compilation_set(&self) -> CompilationSet {
        let mut all_sources: Vec<_> = self.external_sources.iter().map(|x| CompilationSource::Library(x.1.clone())).collect();
        let mut sources: Vec<_> = self.sources.iter().map(|x| CompilationSource::Text((x.1.clone(), x.0.clone()))).collect();

        all_sources.append(&mut sources);

        CompilationSet {
            sources: all_sources
        }
    }

    fn on_did_change_text_document(&mut self, file_id: &FileId, content: &str) -> Option<Diagnostic> {
        match self.sources.iter().position(|val| val.0 == *file_id) {
            Some(index) => self.sources[index] = (file_id.clone(), content.to_owned()),
            None => self.sources.push((file_id.clone(), content.to_owned())),
        }

        analyze(&self.compilation_set()).err()
    }
}