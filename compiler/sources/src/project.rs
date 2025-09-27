//! Project management for multiple source files

use std::{collections::HashMap, fs, path::Path};

use ironplc_dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::{info, trace, warn};

use crate::{file_type::FileType, source::Source};

/// A project consisting of one or more source files
pub struct SourceProject {
    /// The source files in the project
    sources: HashMap<FileId, Source>,
}

impl Default for SourceProject {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceProject {
    /// Create a new empty project
    pub fn new() -> Self {
        SourceProject {
            sources: HashMap::new(),
        }
    }

    /// Add a source file to the project by file ID
    pub fn add_file(&mut self, file_id: FileId) -> Result<(), Diagnostic> {
        match Source::try_from_file_id(&file_id) {
            Ok(src) => {
                self.add_source(file_id, src.as_string().to_string());
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Add source content directly to the project
    pub fn add_source(&mut self, file_id: FileId, content: String) {
        trace!("Adding source file: {}", file_id);
        let source = Source::new(content, &file_id);
        self.sources.insert(file_id, source);
    }

    /// Get a source by file ID
    pub fn get_source(&self, file_id: &FileId) -> Option<&Source> {
        self.sources.get(file_id)
    }

    /// Get a mutable source by file ID
    pub fn get_source_mut(&mut self, file_id: &FileId) -> Option<&mut Source> {
        self.sources.get_mut(file_id)
    }

    /// Get all sources
    pub fn sources(&self) -> Vec<&Source> {
        self.sources.values().collect()
    }

    /// Get all sources mutably
    pub fn sources_mut(&mut self) -> Vec<&mut Source> {
        self.sources.values_mut().collect()
    }

    /// Initialize project from a directory, loading all supported files
    pub fn initialize_from_directory(&mut self, dir: &Path) -> Vec<Diagnostic> {
        info!("Initializing project from directory: {}", dir.display());

        self.sources.clear();

        match fs::read_dir(dir) {
            Ok(files) => {
                let mut errors = vec![];

                files
                    .filter_map(Result::ok)
                    .filter(|f| {
                        let file_type = FileType::from_path(&f.path());
                        file_type.is_supported()
                    })
                    .for_each(|f| {
                        let file_id = FileId::from_dir_entry(f);
                        if let Err(err) = self.add_file(file_id) {
                            errors.push(err);
                        }
                    });

                errors
            }
            Err(err) => {
                warn!("Unable to read directory '{}': {}", dir.display(), err);
                vec![Diagnostic::problem(
                    Problem::CannotReadDirectory,
                    Label::file(FileId::from_path(dir), err.to_string()),
                )]
            }
        }
    }

    /// Remove a source file from the project
    pub fn remove_source(&mut self, file_id: &FileId) -> Option<Source> {
        self.sources.remove(file_id)
    }

    /// Clear all sources from the project
    pub fn clear(&mut self) {
        self.sources.clear();
    }

    /// Get the number of sources in the project
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Check if the project is empty
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    #[test]
    fn new_project_is_empty() {
        let project = SourceProject::new();
        assert!(project.is_empty());
        assert_eq!(project.len(), 0);
    }

    #[test]
    fn add_source_increases_count() {
        let mut project = SourceProject::new();
        let file_id = FileId::from_string("test.st");
        project.add_source(file_id.clone(), "PROGRAM Main END_PROGRAM".to_string());

        assert_eq!(project.len(), 1);
        assert!(!project.is_empty());
        assert!(project.get_source(&file_id).is_some());
    }

    #[test]
    fn remove_source_decreases_count() {
        let mut project = SourceProject::new();
        let file_id = FileId::from_string("test.st");
        project.add_source(file_id.clone(), "PROGRAM Main END_PROGRAM".to_string());

        let removed = project.remove_source(&file_id);
        assert!(removed.is_some());
        assert_eq!(project.len(), 0);
        assert!(project.is_empty());
    }

    #[test]
    fn clear_removes_all_sources() {
        let mut project = SourceProject::new();
        project.add_source(FileId::from_string("test1.st"), "content1".to_string());
        project.add_source(FileId::from_string("test2.xml"), "content2".to_string());

        assert_eq!(project.len(), 2);
        project.clear();
        assert_eq!(project.len(), 0);
        assert!(project.is_empty());
    }

    #[test]
    fn sources_returns_all_sources() {
        let mut project = SourceProject::new();
        project.add_source(FileId::from_string("test1.st"), "content1".to_string());
        project.add_source(FileId::from_string("test2.xml"), "content2".to_string());

        let sources = project.sources();
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn default_creates_empty_project() {
        let project = SourceProject::default();
        assert!(project.is_empty());
        assert_eq!(project.len(), 0);
    }

    #[test]
    fn add_file_error_handling() {
        let mut project = SourceProject::new();
        // Try to add a file that doesn't exist
        let result = project.add_file(FileId::from_string("/nonexistent/file.st"));
        assert!(result.is_err());
    }

    #[test]
    fn get_source_mut_returns_mutable_reference() {
        let mut project = SourceProject::new();
        let file_id = FileId::from_string("test.st");
        project.add_source(file_id.clone(), "content".to_string());

        let source_mut = project.get_source_mut(&file_id);
        assert!(source_mut.is_some());

        let nonexistent = project.get_source_mut(&FileId::from_string("nonexistent.st"));
        assert!(nonexistent.is_none());
    }

    #[test]
    fn sources_mut_returns_mutable_references() {
        let mut project = SourceProject::new();
        project.add_source(FileId::from_string("test1.st"), "content1".to_string());
        project.add_source(FileId::from_string("test2.xml"), "content2".to_string());

        let sources_mut = project.sources_mut();
        assert_eq!(sources_mut.len(), 2);
    }

    #[test]
    fn initialize_from_nonexistent_directory() {
        let mut project = SourceProject::new();
        let diagnostics = project.initialize_from_directory(Path::new("/nonexistent/directory"));

        assert!(!diagnostics.is_empty());
        assert!(project.is_empty());
    }
}
