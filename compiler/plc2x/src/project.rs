//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use ironplc_dsl::{common::Library, core::FileId, diagnostic::Diagnostic};
use ironplc_parser::token::TokenType;

use crate::{
    compilation_set::{CompilationSet, CompilationSource},
    stages::{analyze, tokenize},
};

/// A project consisting of one or more files.
///
/// The project acts is akin to an interface for interacting with the compiler
/// for one or more files.
pub trait Project {
    /// Updates the text for a document.
    fn change_text_document(&mut self, file_id: &FileId, content: &str);

    /// Requests tokens for the file.
    fn tokenize(&self, file_id: &FileId) -> Result<Vec<TokenType>, Vec<Diagnostic>>;

    /// Requests semantic analysis for the project.
    fn semantic(&self) -> Result<(), Vec<Diagnostic>>;

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

impl Default for FileBackedProject {
    fn default() -> Self {
        Self::new()
    }
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
        let mut all_sources: Vec<_> = self
            .external_sources
            .iter()
            .map(|x| CompilationSource::Library(x.1.clone()))
            .collect();
        let mut sources: Vec<_> = self
            .sources
            .iter()
            .map(|x| CompilationSource::Text((x.1.clone(), x.0.clone())))
            .collect();

        all_sources.append(&mut sources);

        CompilationSet {
            sources: all_sources,
            references: vec![],
        }
    }

    fn change_text_document(&mut self, file_id: &FileId, content: &str) {
        match self.sources.iter().position(|val| val.0 == *file_id) {
            Some(index) => self.sources[index] = (file_id.clone(), content.to_owned()),
            None => self.sources.push((file_id.clone(), content.to_owned())),
        }
    }

    fn tokenize(&self, file_id: &FileId) -> Result<Vec<TokenType>, Vec<Diagnostic>> {
        let result: Option<Result<Vec<TokenType>, Vec<Diagnostic>>> =
            self.sources.iter().find_map(|item| {
                if item.0 != *file_id {
                    return None;
                }
                Some(tokenize(item.1.as_str(), file_id))
            });

        if let Some(result) = result {
            return result;
        }
        Ok(vec![])
    }

    fn semantic(&self) -> Result<(), Vec<Diagnostic>> {
        let compilation_set = self.compilation_set();
        analyze(&compilation_set)
    }
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::FileId;

    use super::{FileBackedProject, Project};

    #[test]
    fn compilation_set_when_empty_then_ok() {
        let project = FileBackedProject::default();
        assert_eq!(0, project.compilation_set().sources.len());
        assert_eq!(0, project.compilation_set().references.len());
    }

    #[test]
    fn analyze_when_not_valid_then_err() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA");
    }
}
