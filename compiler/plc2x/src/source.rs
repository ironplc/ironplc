//! Implements capabilities to read source files from disk.
//!
//! This module handles source encodings.

use std::{
    borrow::Borrow,
    path::{Path, PathBuf},
};

use ironplc_dsl::{
    common::Library,
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use log::{debug, trace};

/// The contents of a source file.
#[derive(Debug)]
pub struct Source {
    file_id: FileId,
    data: String,
    library: Option<Result<Library, Diagnostic>>,
}

impl Source {
    pub fn new(source: String, file_id: &FileId) -> Self {
        Self {
            file_id: file_id.clone(),
            data: source,
            library: None,
        }
    }
    pub fn try_from_file_id(item: &FileId) -> Result<Source, Diagnostic> {
        let path: PathBuf = item.to_string().into();
        path_to_source(&path).map(|src| Source::new(src, item))
    }
    pub fn as_string(&self) -> &str {
        self.data.borrow()
    }
    pub fn file_id(&self) -> &FileId {
        &self.file_id
    }
    pub fn library(&mut self) -> Result<&Library, Vec<&Diagnostic>> {
        if self.library.is_none() {
            self.library = Some(parse_program(self.data.borrow(), &self.file_id))
        }

        match &self.library {
            Some(result) => match result {
                Ok(library) => Ok(library),
                Err(diagnostic) => Err(vec![diagnostic]),
            },
            None => {
                // this will crash the program. While it is should not be possible to
                // get here, this is a bad practice but I need to refactor source first.
                todo!()
            }
        }
    }
}

/// Creates a compilation source item from the path (by reading the file).
fn path_to_source(path: &Path) -> Result<String, Diagnostic> {
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
