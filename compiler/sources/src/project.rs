//! Project management for multiple source files

use std::{collections::HashMap, path::Path};

use ironplc_dsl::{core::FileId, diagnostic::Diagnostic};
use log::{info, trace};

use crate::source::Source;

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

    /// Initialize project from a directory using project discovery.
    ///
    /// Detects existing project structures (Beremiz, TwinCAT) and loads
    /// the appropriate set of files. Falls back to enumerating all
    /// supported files when no specific project is detected.
    pub fn initialize_from_directory(&mut self, dir: &Path) -> Vec<Diagnostic> {
        info!("Initializing project from directory: {}", dir.display());

        self.sources.clear();

        let discovered = match crate::discovery::discover(dir) {
            Ok(project) => project,
            Err(diag) => return vec![diag],
        };

        info!(
            "Discovered {:?} project with {} files",
            discovered.project_type,
            discovered.files.len()
        );

        let mut errors = vec![];
        for file_path in &discovered.files {
            let file_id = FileId::from_path(file_path);
            if let Err(err) = self.add_file(file_id) {
                errors.push(err);
            }
        }
        errors
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
    fn get_source_when_added_by_file_id_then_returns_the_item() {
        let mut project = SourceProject::new();
        let file_id = FileId::from_string("test.st");
        project.add_source(file_id.clone(), "content".to_string());

        // Get the source and verify FileId equality
        let source = project.get_source(&file_id).unwrap();

        // Both the HashMap key and the Source's internal file_id should be equal
        assert_eq!(file_id, *source.file_id());
    }

    #[test]
    fn initialize_from_nonexistent_directory() {
        let mut project = SourceProject::new();
        let diagnostics = project.initialize_from_directory(Path::new("/nonexistent/directory"));

        assert!(!diagnostics.is_empty());
        assert!(project.is_empty());
    }

    #[test]
    fn mixed_xml_and_st_sources_parse_successfully() {
        let mut project = SourceProject::new();

        // Add ST source defining a type
        let st_content = r#"
TYPE
  Counter : INT := 0;
END_TYPE

FUNCTION_BLOCK FB_Counter
VAR
  count : Counter;
END_VAR
END_FUNCTION_BLOCK
"#;
        project.add_source(FileId::from_string("types.st"), st_content.to_string());

        // Add XML source defining a program
        let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous>
      <pou name="MainProgram" pouType="program">
        <interface>
          <localVars>
            <variable name="x"><type><INT/></type></variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">x := x + 1;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;
        project.add_source(FileId::from_string("main.xml"), xml_content.to_string());

        // Both sources should parse successfully
        assert_eq!(project.len(), 2);

        // Parse the ST source
        let st_source = project
            .get_source_mut(&FileId::from_string("types.st"))
            .unwrap();
        let st_library = st_source.library();
        assert!(
            st_library.is_ok(),
            "ST source should parse successfully: {:?}",
            st_library.err()
        );
        let st_lib = st_library.unwrap();
        assert_eq!(st_lib.elements.len(), 2); // 1 type + 1 function block

        // Parse the XML source
        let xml_source = project
            .get_source_mut(&FileId::from_string("main.xml"))
            .unwrap();
        let xml_library = xml_source.library();
        assert!(xml_library.is_ok(), "XML source should parse successfully");
        let xml_lib = xml_library.unwrap();
        assert_eq!(xml_lib.elements.len(), 1); // 1 program
    }
}
