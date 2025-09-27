//! Source file abstraction and management

use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
};

use ironplc_dsl::{
    common::Library,
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::{debug, trace};

use crate::{file_type::FileType, parsers};

/// The contents of a source file with parsing capabilities
#[derive(Debug)]
pub struct Source {
    file_id: FileId,
    data: String,
    file_type: FileType,
    library: Option<Result<Library, Diagnostic>>,
}

impl Source {
    /// Create a new Source from content and file ID
    pub fn new(source: String, file_id: &FileId) -> Self {
        let path: PathBuf = file_id.to_string().into();
        let file_type = FileType::from_path(&path);

        Self {
            file_id: file_id.clone(),
            data: source,
            file_type,
            library: None,
        }
    }

    /// Create a Source by reading from a file
    pub fn try_from_file_id(item: &FileId) -> Result<Source, Diagnostic> {
        let path: PathBuf = item.to_string().into();
        let content = read_file_content(&path)?;
        Ok(Source::new(content, item))
    }

    /// Get the raw content as a string
    pub fn as_string(&self) -> &str {
        self.data.borrow()
    }

    /// Get the file ID
    pub fn file_id(&self) -> &FileId {
        &self.file_id
    }

    /// Get the detected file type
    pub fn file_type(&self) -> FileType {
        self.file_type.clone()
    }

    /// Parse the source into a Library, caching the result
    pub fn library(&mut self) -> Result<&Library, Vec<Diagnostic>> {
        if self.library.is_none() {
            self.library = Some(self.parse_content());
        }

        match &self.library {
            Some(result) => match result {
                Ok(library) => Ok(library),
                Err(diagnostic) => Err(vec![diagnostic.clone()]),
            },
            None => {
                // This should not be possible to reach since we set self.library above
                Err(vec![Diagnostic::internal_error(file!(), line!())])
            }
        }
    }

    /// Parse the content using the appropriate parser
    fn parse_content(&self) -> Result<Library, Diagnostic> {
        parsers::parse_source(self.file_type.clone(), &self.data, &self.file_id)
    }
}

/// Read file content with encoding detection
fn read_file_content(path: &Path) -> Result<String, Diagnostic> {
    debug!("Reading file {}", path.display());

    let bytes = std::fs::read(path)
        .map_err(|e| diagnostic(Problem::CannotReadFile, path, e.to_string()))?;

    // We try different encoders and return the first one that matches. From section 2.1.1,
    // the allowed character set is one with characters consistent with ISO/IEC 10646-1 (UCS).
    // There are other valid encodings, so if encountered, it is reasonable to add more here.
    let decoders: [&'static encoding_rs::Encoding; 2] =
        [encoding_rs::UTF_8, encoding_rs::WINDOWS_1252];

    let result = decoders.into_iter().find_map(move |d| {
        let (res, encoding_used, had_errors) = d.decode(&bytes);
        if had_errors {
            trace!(
                "Path {} did not match encoding {}",
                path.display(),
                encoding_used.name()
            );
            return None;
        }
        trace!(
            "Path {} matched encoding {}",
            path.display(),
            encoding_used.name()
        );
        Some(res.to_string())
    });

    match result {
        Some(res) => Ok(res),
        None => Err(diagnostic(
            Problem::UnsupportedEncoding,
            path,
            String::from("The file is not UTF-8 or latin1"),
        )),
    }
}

fn diagnostic(problem: Problem, path: &Path, message: String) -> Diagnostic {
    Diagnostic::problem(problem, Label::file(FileId::from_path(path), message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    #[test]
    fn source_new_detects_file_type() {
        let st_file_id = FileId::from_string("test.st");
        let source = Source::new("PROGRAM Main END_PROGRAM".to_string(), &st_file_id);
        assert_eq!(source.file_type(), FileType::StructuredText);

        let xml_file_id = FileId::from_string("test.xml");
        let source = Source::new("<xml></xml>".to_string(), &xml_file_id);
        assert_eq!(source.file_type(), FileType::Xml);
    }

    #[test]
    fn source_as_string_returns_content() {
        let file_id = FileId::from_string("test.st");
        let content = "PROGRAM Main END_PROGRAM";
        let source = Source::new(content.to_string(), &file_id);
        assert_eq!(source.as_string(), content);
    }

    #[test]
    fn xml_source_returns_empty_library() {
        let file_id = FileId::from_string("test.xml");
        let content = r#"<?xml version="1.0"?><project></project>"#;
        let mut source = Source::new(content.to_string(), &file_id);

        let result = source.library();
        assert!(result.is_ok());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 0);
    }

    #[test]
    fn try_from_file_id_with_nonexistent_file() {
        let file_id = FileId::from_string("/nonexistent/file.st");
        let result = Source::try_from_file_id(&file_id);
        assert!(result.is_err());
    }

    #[test]
    fn source_with_unknown_file_type() {
        let file_id = FileId::from_string("test.unknown");
        let mut source = Source::new("some content".to_string(), &file_id);

        let result = source.library();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn source_file_id_returns_correct_id() {
        let file_id = FileId::from_string("test.st");
        let source = Source::new("content".to_string(), &file_id);
        assert_eq!(source.file_id(), &file_id);
    }

    #[test]
    fn source_caches_library_result() {
        let file_id = FileId::from_string("test.xml");
        let mut source = Source::new("content".to_string(), &file_id);

        // First call should parse and cache
        {
            let result1 = source.library();
            assert!(result1.is_ok());
        }

        // Second call should return cached result
        {
            let result2 = source.library();
            assert!(result2.is_ok());
        }

        // Verify the library is cached by checking it's not None
        assert!(source.library.is_some());
    }
}
