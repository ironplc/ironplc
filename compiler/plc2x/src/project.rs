//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use std::path::Path;

use ironplc_analyzer::stages::analyze;
use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::{options::ParseOptions, token::Token, tokenize_program};
use ironplc_problems::Problem;
use ironplc_sources::{Source, SourceProject};
use log::trace;

/// A project consisting of one or more files.
///
/// The project acts is akin to an interface for interacting with the compiler
/// for one or more files.
pub trait Project {
    /// Initialize
    fn initialize(&mut self, dir: &Path) -> Vec<Diagnostic>;

    /// Updates the text for a document.
    fn change_text_document(&mut self, file_id: &FileId, content: String);

    /// Requests tokens for the file.
    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>);

    /// Requests semantic analysis for the project.
    fn semantic(&mut self) -> Result<(), Vec<Diagnostic>>;

    /// Gets the sources that are the project.
    fn sources(&self) -> Vec<&Source>;

    fn sources_mut(&mut self) -> Vec<&mut Source>;

    fn find(&self, file_id: &FileId) -> Option<&Source>;
}

/// A project is a collection of files used together as a single unit.
pub struct FileBackedProject {
    /// The underlying source project
    source_project: SourceProject,
}

impl Default for FileBackedProject {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBackedProject {
    pub fn new() -> Self {
        FileBackedProject {
            source_project: SourceProject::new(),
        }
    }

    pub fn push(&mut self, file_id: FileId) -> Result<(), Diagnostic> {
        self.source_project.add_file(file_id)
    }

    pub fn get(&self, file_id: &FileId) -> Option<&Source> {
        self.source_project.get_source(file_id)
    }
}

impl Project for FileBackedProject {
    /// Create a new project from the files in the specified directory.
    fn initialize(&mut self, dir: &Path) -> Vec<Diagnostic> {
        self.source_project.initialize_from_directory(dir)
    }

    fn change_text_document(&mut self, file_id: &FileId, content: String) {
        trace!(
            "Change text document sources initial length is {}",
            self.source_project.len()
        );

        self.source_project.add_source(file_id.clone(), content);

        trace!(
            "Change text document sources new length is {}",
            self.source_project.len()
        );
    }

    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
        let source = self.source_project.get_source(file_id);

        match source {
            Some(src) => tokenize_program(src.as_string(), file_id, &ParseOptions::default()),
            None => (
                vec![],
                vec![Diagnostic::problem(
                    Problem::NoContent,
                    Label::span(SourceSpan::default(), "No documents to tokenize"),
                )],
            ),
        }
    }

    fn semantic(&mut self) -> Result<(), Vec<Diagnostic>> {
        // We would like to do "best effort" semantic analysis. So, we will do
        // semantic analysis on the items we can analyze, and the provide full
        // diagnostics for any problems
        let mut all_libraries = vec![];
        let mut all_diagnostics: Vec<Diagnostic> = vec![];

        // Process each source individually to avoid borrowing issues
        for source in self.source_project.sources_mut() {
            match source.library() {
                Ok(library) => {
                    all_libraries.push(library);
                }
                Err(diagnostics) => {
                    for diagnostic in diagnostics {
                        all_diagnostics.push(diagnostic.clone());
                    }
                }
            }
        }

        // Do the analysis
        match analyze(&all_libraries) {
            Ok(_) => Ok(()),
            Err(diagnostics) => {
                // If we had an error, then add more diagnostics to any that we already had
                all_diagnostics.extend(diagnostics);
                Err(all_diagnostics)
            }
        }
    }

    fn sources(&self) -> Vec<&Source> {
        self.source_project.sources()
    }

    fn sources_mut(&mut self) -> Vec<&mut Source> {
        self.source_project.sources_mut()
    }

    fn find(&self, file_id: &FileId) -> Option<&Source> {
        self.source_project.get_source(file_id)
    }
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::FileId;

    use super::{FileBackedProject, Project};

    #[test]
    fn change_text_document_when_overwrite_then_one_file() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA".to_owned());
        project.change_text_document(&FileId::default(), "BBB".to_owned());
        assert_eq!(1, project.sources().len());
    }

    #[test]
    fn compilation_set_when_empty_then_ok() {
        let project = FileBackedProject::default();
        assert_eq!(0, project.sources().len());
    }

    #[test]
    fn tokenize_when_has_other_file_then_error() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA".to_owned());
        let res = project.tokenize(&FileId::from_string("abc"));
        assert!(!res.1.is_empty());
    }

    #[test]
    fn analyze_when_not_valid_then_err() {
        let mut project = FileBackedProject::default();
        project.change_text_document(&FileId::default(), "AAA".to_owned());
    }

    #[test]
    fn xml_file_returns_empty_library() {
        let mut project = FileBackedProject::default();
        let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <name>Test Project</name>
</project>"#;

        let file_id = FileId::from_string("test.xml");
        project.change_text_document(&file_id, xml_content.to_owned());

        let source = project.sources_mut().into_iter().next().unwrap();
        let library_result = source.library();

        assert!(library_result.is_ok());
        let library = library_result.unwrap();
        assert_eq!(0, library.elements.len()); // Should be empty
    }
}
