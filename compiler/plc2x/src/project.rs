//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use std::{collections::HashMap, fs, path::Path};

use ironplc_analyzer::stages::analyze;
use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::{options::ParseOptions, token::Token, tokenize_program};
use ironplc_problems::Problem;
use log::{info, trace, warn};

use crate::source::Source;

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
    /// The user-supplied source files for the project
    sources: HashMap<FileId, Source>,
}

impl Default for FileBackedProject {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBackedProject {
    pub fn new() -> Self {
        FileBackedProject {
            sources: HashMap::new(),
        }
    }

    pub fn push(&mut self, file_id: FileId) -> Result<(), Diagnostic> {
        match Source::try_from_file_id(&file_id) {
            Ok(src) => {
                self.change_text_document(&file_id, src.as_string().to_string());
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn get(&self, file_id: &FileId) -> Option<&Source> {
        self.sources.get(file_id)
    }
}

impl Project for FileBackedProject {
    /// Create a new project from the files in the specified directory.
    ///
    /// TODO this is not definitely not the right architecture.
    fn initialize(&mut self, dir: &Path) -> Vec<Diagnostic> {
        info!("Initialize project from path {:?}", dir);

        self.sources.clear();

        match fs::read_dir(dir) {
            Ok(files) => {
                let mut errors = vec![];

                files
                    .filter_map(Result::ok)
                    .filter(|f| {
                        f.path().extension().is_some_and(|ext| {
                            ext.eq_ignore_ascii_case("st") || ext.eq_ignore_ascii_case("iec")
                        })
                    })
                    .for_each(|f| {
                        let path = FileId::from_dir_entry(f);
                        match self.push(path) {
                            Ok(_) => {}
                            Err(err) => errors.push(err),
                        }
                    });

                errors
            }
            Err(err) => {
                warn!("Unable to read directory '{}': {}", dir.display(), err);
                let problem = Diagnostic::problem(
                    Problem::CannotReadDirectory,
                    Label::file(FileId::from_path(dir), err.to_string()),
                );
                vec![problem]
            }
        }
    }

    fn change_text_document(&mut self, file_id: &FileId, content: String) {
        trace!(
            "Change text document sources initial length is {}",
            self.sources.len()
        );

        self.sources
            .insert(file_id.clone(), Source::new(content, file_id));

        trace!(
            "Change text document sources new length is {}",
            self.sources.len()
        );
    }

    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
        trace!("Sources are {:?}", self.sources);
        let source = self.sources.get(file_id);

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
        let library_results: Vec<_> = self
            .sources
            .iter_mut()
            .map(|source| source.1.library())
            .collect();

        // We would like to do "best effort" semantic analysis. So, we will do
        // semantic analysis on the items we can analyze, and the provide full
        // diagnostics for any problems
        let mut all_libraries = vec![];
        let mut all_diagnostics: Vec<Diagnostic> = vec![];
        for library_result in library_results {
            match library_result {
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
        self.sources.values().collect()
    }

    fn sources_mut(&mut self) -> Vec<&mut Source> {
        self.sources.values_mut().collect()
    }

    fn find(&self, file_id: &FileId) -> Option<&Source> {
        self.sources.get(file_id)
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
}
