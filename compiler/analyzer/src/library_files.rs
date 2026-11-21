//! The library each source file of an analysis belongs to.
//!
//! A library here is either a compatibility library the build activated
//! (`Tc2_System`, see `specs/design/compatibility-libraries.md`) or a project
//! loaded alongside the one being compiled, which satisfies another project's
//! reference to it by its name (`REQ-CL-sources-009`). Every declaration's
//! span names the file it was written in, so this record says which library
//! a declaration came from. A file that belongs to no library, such as a
//! source given on its own, is not recorded.
//!
//! Names compare as identifiers do, ignoring case.

use std::collections::HashMap;

use ironplc_dsl::core::{FileId, Id};

/// Which library each recorded source file belongs to.
#[derive(Clone, Debug, Default)]
pub struct LibraryFiles {
    libraries: HashMap<FileId, Id>,
}

impl LibraryFiles {
    /// A record of no files.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `file` belongs to the library named `library`. A file
    /// recorded again keeps the library it was recorded with first.
    pub fn insert(&mut self, file: FileId, library: &str) {
        self.libraries
            .entry(file)
            .or_insert_with(|| Id::from(library));
    }

    /// The name of the library `file` belongs to, or `None` for a file that
    /// belongs to none.
    pub fn library_of(&self, file: &FileId) -> Option<&Id> {
        self.libraries.get(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::analyze_with_library_files;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    #[test]
    fn analyze_with_library_files_when_files_recorded_then_context_keeps_them() {
        let library_file = FileId::from_string("libs/Tc2_System.st");
        let library = parse_program(
            "FUNCTION LIB_ONE : INT LIB_ONE := 1; END_FUNCTION",
            &library_file,
            &CompilerOptions::default(),
        )
        .unwrap();
        let user = parse_program(
            "PROGRAM main VAR x : INT; END_VAR x := LIB_ONE(); END_PROGRAM",
            &FileId::from_string("main.st"),
            &CompilerOptions::default(),
        )
        .unwrap();
        let mut files = LibraryFiles::new();
        files.insert(library_file.clone(), "Tc2_System");

        let (_, context) =
            analyze_with_library_files(&[&library, &user], &files, &CompilerOptions::default())
                .unwrap();

        assert!(!context.has_diagnostics(), "{:?}", context.diagnostics());
        assert_eq!(
            context.library_files().library_of(&library_file),
            Some(&Id::from("Tc2_System"))
        );
        assert_eq!(
            context
                .library_files()
                .library_of(&FileId::from_string("main.st")),
            None
        );
    }

    #[test]
    fn library_of_when_file_recorded_then_its_library() {
        let mut files = LibraryFiles::new();
        files.insert(FileId::from_string("libs/Tc2_System.st"), "Tc2_System");

        assert_eq!(
            files.library_of(&FileId::from_string("libs/Tc2_System.st")),
            Some(&Id::from("tc2_system"))
        );
    }

    #[test]
    fn library_of_when_file_not_recorded_then_none() {
        let files = LibraryFiles::new();

        assert_eq!(files.library_of(&FileId::from_string("main.st")), None);
    }

    #[test]
    fn insert_when_file_recorded_twice_then_first_library_kept() {
        let mut files = LibraryFiles::new();
        files.insert(FileId::from_string("shared.st"), "Main");
        files.insert(FileId::from_string("shared.st"), "BROTLib");

        assert_eq!(
            files.library_of(&FileId::from_string("shared.st")),
            Some(&Id::from("Main"))
        );
    }
}
