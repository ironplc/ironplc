//! Paths to the golden project trees the discovery tests run against.
//!
//! Discovery's job is to decide what a path on disk means -- which
//! manifest a folder holds, whether it holds more than one, whether a
//! `<Compile>` entry names a file that exists. Those questions are about
//! real files, so the tests point at real files: one checked-in tree per
//! case under `resources/test/discovery`, laid out the way TcXaeShell
//! lays a solution out.
//!
//! Nothing is built at test time. A tree assembled by a helper would
//! agree with the parser by construction and could never catch it
//! drifting from the format an IDE actually writes; a tree checked in is
//! reviewable, and `git diff` shows when its meaning changes.

use std::path::{Path, PathBuf};

/// The golden tree named `name`, e.g. `sln_chain`.
pub(super) fn tree(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("resources");
    path.push("test");
    path.push("discovery");
    path.push(name);
    path
}

/// A file inside the golden tree named `name`, e.g.
/// `tree_file("sln_chain", "Solution.sln")`. `relative` uses `/`
/// separators on every platform.
pub(super) fn tree_file(name: &str, relative: &str) -> PathBuf {
    let mut path = tree(name);
    for segment in relative.split('/') {
        path.push(segment);
    }
    path
}

/// Whether `path` sits inside the golden tree named `name` -- for
/// assertions about which project a file was resolved from.
pub(super) fn in_tree(path: &Path, name: &str) -> bool {
    path.starts_with(tree(name))
}
