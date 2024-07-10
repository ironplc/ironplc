use std::{fs, path::PathBuf};

pub fn read_shared_resource(name: &'static str) -> String {
    fs::read_to_string(shared_resource_path(name)).expect("Unable to read file")
}

pub fn shared_resource_path(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("resources");
    path.push("test");
    path.push(name);
    path
}
