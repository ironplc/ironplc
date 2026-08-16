//! Test plumbing for the discovery tests.
//!
//! Discovery's job is to decide which files on disk make up a project --
//! which manifest a directory holds, whether there is more than one,
//! whether a `<Compile>` entry names a file that exists. Those questions
//! are about the filesystem, so the tests that ask them need a real tree
//! under a `TempDir`.
//!
//! What lives here is only the plumbing for laying one out. Manifest
//! *content* is written literally at each test site: a builder that emits
//! the handful of lines the parser already reads would agree with the
//! parser by construction, and could never catch the parser drifting from
//! the format a real IDE writes. The format itself is exercised against
//! literal manifest text in `sln.rs`, with no file involved at all.

use std::fs;
use std::path::Path;

/// Write `content` to `path`, creating any missing parent directories.
pub(super) fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

/// Write a `.plcproj` at `path` declaring one `<Compile>` entry per name
/// in `sources`, and create each named file alongside it.
///
/// The `<Compile>` entries have to agree with the files that exist for
/// the project to resolve at all, so the two are written together --
/// this states a tree, not a file format.
pub(super) fn write_plcproj(path: &Path, sources: &[&str]) {
    let entries: String = sources
        .iter()
        .map(|source| format!("    <Compile Include=\"{source}\" />\n"))
        .collect();
    write_file(
        path,
        &format!(
            "<Project xmlns=\"http://schemas.microsoft.com/developer/msbuild/2003\">\n  <ItemGroup>\n{entries}  </ItemGroup>\n</Project>"
        ),
    );

    let dir = path.parent().unwrap();
    for source in sources {
        write_file(&dir.join(source.replace('\\', "/")), "<TcPlcObject/>");
    }
}
