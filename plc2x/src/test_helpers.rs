use ironplc_dsl::common::*;

use std::fs;
use std::path::PathBuf;

#[cfg(test)]
pub fn read_resource(name: &'static str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);

    fs::read_to_string(path).expect("Unable to read file")
}

#[cfg(test)]
pub fn new_library<T>(element: LibraryElement) -> Result<Library, T> {
    Ok(Library {
        elements: vec![element],
    })
}
