//! Tests that a project's analysis knows which library each source file
//! belongs to: a bundled compatibility library, a pre-parsed one, or a
//! project loaded alongside.

use ironplc_dsl::core::{FileId, Id};
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::LibraryName;

use super::{FileBackedProject, MemoryBackedProject, PreparsedLibrary, Project};

const MAIN: &str = "PROGRAM main VAR x : INT; END_VAR x := 1; END_PROGRAM";

/// The name of the library analysis recorded for `file`.
fn library_of(project: &dyn Project, file: &FileId) -> Option<Id> {
    project
        .semantic_context()
        .expect("analysis must produce a context")
        .library_files()
        .library_of(file)
        .cloned()
}

#[test]
fn semantic_when_bundled_library_activated_then_its_files_belong_to_it() {
    let mut project = FileBackedProject::with_options(CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    });
    project.set_activated_libraries(vec![LibraryName::from("Tc2_System")]);
    project.change_text_document(&FileId::from_string("main.st"), MAIN.to_owned());
    let (loaded, _) = project.load_activated_libraries();
    let library_file = loaded[0].files[0].clone();

    project.semantic();

    assert_eq!(
        library_of(&project, &library_file),
        Some(Id::from("Tc2_System"))
    );
    assert_eq!(library_of(&project, &FileId::from_string("main.st")), None);
}

#[test]
fn semantic_when_file_project_set_then_file_belongs_to_project() {
    let mut project = FileBackedProject::default();
    project.change_text_document(&FileId::from_string("brot.st"), MAIN.to_owned());
    project.set_file_project(FileId::from_string("brot.st"), "BROTLib");

    project.semantic();

    assert_eq!(
        library_of(&project, &FileId::from_string("brot.st")),
        Some(Id::from("BROTLib"))
    );
}

#[test]
fn semantic_when_preparsed_library_supplied_then_its_file_belongs_to_it() {
    let file_id = FileId::from_string("libs/Tc2_Math/Tc2_Math.st");
    let library = ironplc_sources::parse_source(
        ironplc_sources::FileType::StructuredText,
        "FUNCTION LIB_ONE : INT LIB_ONE := 1; END_FUNCTION",
        &file_id,
        &CompilerOptions::default(),
    )
    .unwrap();
    let mut project = MemoryBackedProject::new(CompilerOptions::default());
    project.set_preparsed_libraries(vec![PreparsedLibrary {
        name: LibraryName::from("Tc2_Math"),
        file_id: file_id.clone(),
        library,
    }]);
    project.add_source(FileId::from_string("main.st"), MAIN.to_owned());

    project.semantic();

    assert_eq!(library_of(&project, &file_id), Some(Id::from("Tc2_Math")));
}
