//! Build script that generates `spec_requirements.rs` from requirement markers
//! in the MCP server design spec.
//!
//! Markers are bold inline IDs of the form `**REQ-STL-001**`, `**REQ-TOL-010**`,
//! or `**REQ-ARC-001**`. The generated file contains one constant per requirement,
//! an `ALL` slice listing every requirement, and an `UNTESTED` slice of
//! requirements that have no corresponding `#[spec_test]` in any source file.
//!
//! See `specs/design/spec-conformance-testing.md`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let specs_dir = Path::new(&manifest_dir).join("../../specs/design");

    let spec_files = ["mcp-server.md"];

    let mut requirements = BTreeSet::new();

    for filename in &spec_files {
        let path = specs_dir.join(filename);
        println!("cargo:rerun-if-changed={}", path.display());

        if let Ok(content) = fs::read_to_string(&path) {
            extract_requirements(&content, &mut requirements);
        }
    }

    // Scan all .rs files under src/ for spec_test(REQ_ patterns to find tested
    // requirements. This allows tests to live in any file.
    let src_dir = Path::new(&manifest_dir).join("src");
    let mut tested = BTreeSet::new();
    let rs_files = collect_rs_files(&src_dir);
    for path in &rs_files {
        println!("cargo:rerun-if-changed={}", path.display());
        if let Ok(content) = fs::read_to_string(path) {
            extract_tested_requirements(&content, &mut tested);
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("spec_requirements.rs");

    let mut code =
        String::from("// Auto-generated from specs/design/mcp-server.md — do not edit.\n\n");

    for req in &requirements {
        let ident = req.replace('-', "_");
        code.push_str(&format!(
            "#[allow(dead_code)] pub const {ident}: &str = \"{req}\";\n"
        ));
    }

    code.push('\n');
    code.push_str("#[allow(dead_code)]\npub const ALL: &[&str] = &[\n");
    for req in &requirements {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    // UNTESTED: requirements in the spec with no #[spec_test] in any source file
    let untested: Vec<&String> = requirements
        .iter()
        .filter(|r| {
            let ident = r.replace('-', "_");
            !tested.contains(&ident)
        })
        .collect();

    code.push('\n');
    code.push_str("#[allow(dead_code)]\npub const UNTESTED: &[&str] = &[\n");
    for req in &untested {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    fs::write(&dest, code).unwrap();
}

/// Collects all `.rs` files under a directory, recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_rs_files(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files
}

/// Extracts all `**REQ-XX-NNN**` bold markers from markdown content.
fn extract_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    while let Some(start) = rest.find("**REQ-") {
        let after_open = &rest[start + 2..]; // skip opening **
        if let Some(end) = after_open.find("**") {
            let id = &after_open[..end];
            if id.starts_with("REQ-") && id.len() >= 8 {
                out.insert(id.to_string());
            }
            rest = &after_open[end + 2..];
        } else {
            break;
        }
    }
}

/// Extracts requirement identifiers from `#[spec_test(REQ_XX_NNN)]` attributes
/// in Rust source. Collects the underscore-form identifiers (e.g. `REQ_STL_001`).
fn extract_tested_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    let needle = "spec_test(REQ_";
    while let Some(start) = rest.find(needle) {
        let after = &rest[start + "spec_test(".len()..];
        if let Some(end) = after.find(')') {
            let ident = &after[..end];
            if ident.starts_with("REQ_") && ident.len() >= 8 {
                out.insert(ident.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
}
