//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use ironplc_dsl::{
    common::Library,
    core::{FileId, SourceLoc},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::token::Token;
use ironplc_problems::Problem;
use log::trace;

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
    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>);

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
        trace!(
            "Change text document sources initial length is {}",
            self.sources.len()
        );
        match self.sources.iter().position(|val| val.0 == *file_id) {
            Some(index) => self.sources[index] = (file_id.clone(), content.to_owned()),
            None => self.sources.push((file_id.clone(), content.to_owned())),
        }
        trace!(
            "Change text document sources new length is {}",
            self.sources.len()
        );
    }

    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
        let result: Option<(Vec<Token>, Vec<Diagnostic>)> = self.sources.iter().find_map(|item| {
            if item.0 != *file_id {
                return None;
            }
            Some(tokenize(item.1.as_str(), file_id))
        });

        match result {
            Some(res) => res,
            None => {
                trace!("Sources are {:?}", self.sources);
                (
                    vec![],
                    vec![Diagnostic::problem(
                        Problem::NoContent,
                        Label::source_loc(&SourceLoc::default(), "No documents to tokenize"),
                    )],
                )
            }
        }
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
    fn change_text_document_when_overwrite_then_one_file() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA");
        project.change_text_document(&FileId::default(), "BBB");
        assert_eq!(1, project.compilation_set().sources.len());
    }

    #[test]
    fn compilation_set_when_empty_then_ok() {
        let project = FileBackedProject::default();
        assert_eq!(0, project.compilation_set().sources.len());
        assert_eq!(0, project.compilation_set().references.len());
    }

    #[test]
    fn tokenize_when_has_other_file_then_error() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA");
        let res = project.tokenize(&FileId::from_string("abc"));
        assert!(!res.1.is_empty());
    }

    #[test]
    fn analyze_when_not_valid_then_err() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA");
    }
}
