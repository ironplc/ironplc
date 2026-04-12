//! Implements a project consisting of one or more files. A project
//! responds to messages (that is, the language server protocol).
//!
//! The trait enables easy testing of the language server protocol integration.

use std::path::Path;

use ironplc_analyzer::{stages::analyze, SemanticContext};
use ironplc_dsl::{
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::{options::CompilerOptions, token::Token, tokenize_program};
use ironplc_problems::Problem;
use ironplc_sources::{Source, SourceProject};
use log::{debug, trace};

/// Runs semantic analysis on the given source project and compiler options.
///
/// This is the shared implementation used by both [`FileBackedProject`] and
/// [`MemoryBackedProject`]. It parses each source into a library, merges them,
/// runs the analyzer, and returns the collected diagnostics plus the semantic
/// context (when type resolution succeeds).
fn run_semantic_analysis(
    source_project: &mut SourceProject,
    compiler_options: &CompilerOptions,
) -> (Result<(), Vec<Diagnostic>>, Option<SemanticContext>) {
    let mut all_libraries = vec![];
    let mut all_diagnostics: Vec<Diagnostic> = vec![];

    for source in source_project.sources_mut() {
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

    match analyze(&all_libraries, compiler_options) {
        Ok((_library, context)) => {
            debug!("Semantic analysis completed {context:?}");
            all_diagnostics.extend(context.diagnostics().iter().cloned());
            let result = if all_diagnostics.is_empty() {
                Ok(())
            } else {
                Err(all_diagnostics)
            };
            (result, Some(context))
        }
        Err(diagnostics) => {
            debug!("Semantic analysis errored {diagnostics:?}");
            all_diagnostics.extend(diagnostics);
            (Err(all_diagnostics), None)
        }
    }
}

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
    ///
    /// Returns `Ok(())` when analysis completes with no diagnostics.
    /// Returns `Err(diagnostics)` when diagnostics are present. Note that the
    /// semantic context may still be cached even when this returns `Err` — use
    /// `semantic_context()` to check availability.
    fn semantic(&mut self) -> Result<(), Vec<Diagnostic>>;

    /// Gets the semantic context from the last analysis.
    ///
    /// Returns `Some` when the last call to `semantic()` succeeded in building
    /// type, function, and symbol environments — even if `semantic()` returned
    /// `Err` due to validation diagnostics. Returns `None` only if `semantic()`
    /// has not been called or if foundational type resolution failed.
    fn semantic_context(&self) -> Option<&SemanticContext>;

    /// Gets the sources that are the project.
    fn sources(&self) -> Vec<&Source>;

    fn sources_mut(&mut self) -> Vec<&mut Source>;

    fn find(&self, file_id: &FileId) -> Option<&Source>;
}

/// A project is a collection of files used together as a single unit.
pub struct FileBackedProject {
    /// The underlying source project
    source_project: SourceProject,
    /// Parse options for this project
    compiler_options: CompilerOptions,
    /// Cached semantic context from the last successful analysis
    semantic_context: Option<SemanticContext>,
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
            compiler_options: CompilerOptions::default(),
            semantic_context: None,
        }
    }

    pub fn with_options(compiler_options: CompilerOptions) -> Self {
        FileBackedProject {
            source_project: SourceProject::with_options(compiler_options),
            compiler_options,
            semantic_context: None,
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
            Some(src) => tokenize_program(src.as_string(), file_id, &self.compiler_options, 0, 0),
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
        self.semantic_context = None;
        let (result, context) =
            run_semantic_analysis(&mut self.source_project, &self.compiler_options);
        self.semantic_context = context;
        result
    }

    fn semantic_context(&self) -> Option<&SemanticContext> {
        self.semantic_context.as_ref()
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

/// An in-memory project that never touches the filesystem.
///
/// This is the project implementation for the MCP server and other contexts
/// where source text is supplied directly rather than read from disk.
pub struct MemoryBackedProject {
    /// The underlying source project
    source_project: SourceProject,
    /// Parse options for this project
    compiler_options: CompilerOptions,
    /// Cached semantic context from the last successful analysis
    semantic_context: Option<SemanticContext>,
}

impl MemoryBackedProject {
    /// Creates a new empty in-memory project with the given compiler options.
    pub fn new(compiler_options: CompilerOptions) -> Self {
        MemoryBackedProject {
            source_project: SourceProject::with_options(compiler_options),
            compiler_options,
            semantic_context: None,
        }
    }

    /// Adds a source to the project by name and content.
    ///
    /// The `file_id` identifies the source in diagnostics. If a source with
    /// the same `file_id` already exists, it is replaced.
    pub fn add_source(&mut self, file_id: FileId, content: String) {
        self.source_project.add_source(file_id, content);
    }
}

impl Project for MemoryBackedProject {
    fn initialize(&mut self, _dir: &Path) -> Vec<Diagnostic> {
        vec![Diagnostic::problem(
            Problem::NoContent,
            Label::span(
                SourceSpan::default(),
                "MemoryBackedProject does not support directory initialization",
            ),
        )]
    }

    fn change_text_document(&mut self, file_id: &FileId, content: String) {
        self.source_project.add_source(file_id.clone(), content);
    }

    fn tokenize(&self, file_id: &FileId) -> (Vec<Token>, Vec<Diagnostic>) {
        let source = self.source_project.get_source(file_id);

        match source {
            Some(src) => tokenize_program(src.as_string(), file_id, &self.compiler_options, 0, 0),
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
        self.semantic_context = None;
        let (result, context) =
            run_semantic_analysis(&mut self.source_project, &self.compiler_options);
        self.semantic_context = context;
        result
    }

    fn semantic_context(&self) -> Option<&SemanticContext> {
        self.semantic_context.as_ref()
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
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use std::path::Path;

    use super::{FileBackedProject, MemoryBackedProject, Project};

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
    fn semantic_when_validation_error_then_context_cached() {
        let mut project = FileBackedProject::default();
        // Valid type declaration with an inverted subrange (semantic error)
        let content = "TYPE\nINVALID_RANGE : INT(10..-10);\nEND_TYPE";
        let file_id = FileId::from_string("test.st");
        project.change_text_document(&file_id, content.to_owned());

        let result = project.semantic();

        assert!(result.is_err());
        assert!(project.semantic_context().is_some());
    }

    #[test]
    fn xml_file_returns_empty_library() {
        let mut project = FileBackedProject::default();
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
    <pous/>
  </types>
</project>"#;

        let file_id = FileId::from_string("test.xml");
        project.change_text_document(&file_id, xml_content.to_owned());

        let source = project.sources_mut().into_iter().next().unwrap();
        let library_result = source.library();

        assert!(library_result.is_ok());
        let library = library_result.unwrap();
        assert_eq!(0, library.elements.len()); // Should be empty
    }

    // MemoryBackedProject tests

    #[test]
    fn memory_add_source_when_valid_then_source_available() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let file_id = FileId::from_string("main.st");
        project.add_source(file_id.clone(), "PROGRAM Main END_PROGRAM".to_owned());

        assert_eq!(1, project.sources().len());
        assert!(project.find(&file_id).is_some());
    }

    #[test]
    fn memory_add_source_when_overwrite_then_one_source() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let file_id = FileId::from_string("main.st");
        project.add_source(file_id.clone(), "AAA".to_owned());
        project.add_source(file_id, "BBB".to_owned());

        assert_eq!(1, project.sources().len());
    }

    #[test]
    fn memory_semantic_when_valid_program_then_ok() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let content = r#"
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#;
        project.add_source(FileId::from_string("main.st"), content.to_owned());

        let result = project.semantic();
        assert!(result.is_ok());
    }

    #[test]
    fn memory_semantic_when_syntax_error_then_err() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        project.add_source(
            FileId::from_string("bad.st"),
            "PROGRAM END_PROGRAM".to_owned(),
        );

        let result = project.semantic();
        assert!(result.is_err());
    }

    #[test]
    fn memory_semantic_when_semantic_error_then_err_with_context() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let content = "TYPE\nINVALID_RANGE : INT(10..-10);\nEND_TYPE";
        project.add_source(FileId::from_string("test.st"), content.to_owned());

        let result = project.semantic();
        assert!(result.is_err());
        assert!(project.semantic_context().is_some());
    }

    #[test]
    fn memory_tokenize_when_valid_then_tokens() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let file_id = FileId::from_string("main.st");
        project.add_source(file_id.clone(), "PROGRAM Main END_PROGRAM".to_owned());

        let (tokens, diagnostics) = project.tokenize(&file_id);
        assert!(!tokens.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn memory_tokenize_when_missing_file_then_error() {
        let project = MemoryBackedProject::new(CompilerOptions::default());
        let (tokens, diagnostics) = project.tokenize(&FileId::from_string("missing.st"));

        assert!(tokens.is_empty());
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn memory_initialize_when_called_then_returns_error() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let diagnostics = project.initialize(Path::new("/some/dir"));

        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn memory_change_text_document_when_called_then_adds_source() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        let file_id = FileId::from_string("main.st");
        project.change_text_document(&file_id, "PROGRAM Main END_PROGRAM".to_owned());

        assert_eq!(1, project.sources().len());
    }

    #[test]
    fn memory_semantic_when_with_dialect_then_uses_options() {
        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let mut project = MemoryBackedProject::new(options);
        let content = r#"
// C-style comment (allowed in Rusty dialect)
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#;
        project.add_source(FileId::from_string("main.st"), content.to_owned());

        let result = project.semantic();
        assert!(result.is_ok());
    }

    #[test]
    fn memory_semantic_when_multiple_sources_then_analyzes_together() {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());

        let fb_content = r#"
FUNCTION_BLOCK Counter
VAR
  count : INT;
END_VAR
  count := count + 1;
END_FUNCTION_BLOCK
"#;
        let program_content = r#"
PROGRAM Main
VAR
  c : Counter;
END_VAR
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#;
        project.add_source(FileId::from_string("counter.st"), fb_content.to_owned());
        project.add_source(FileId::from_string("main.st"), program_content.to_owned());

        // Counter FB from counter.st should be visible to main.st
        let result = project.semantic();
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn memory_semantic_context_when_no_analysis_then_none() {
        let project = MemoryBackedProject::new(CompilerOptions::default());
        assert!(project.semantic_context().is_none());
    }
}
