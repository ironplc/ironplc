use std::path::PathBuf;

#[cfg(test)]
pub fn resource_path(name: &'static str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources/test");
    path.push(name);
    path
}
