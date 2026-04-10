//! Build script that generates `spec_requirements.rs` from requirement markers
//! in the design spec markdown files.
//!
//! Markers are bold inline IDs of the form `**REQ-CF-001**` or `**REQ-IS-001**`.
//! The generated file contains one constant per requirement and an `ALL` slice
//! listing every requirement. See `specs/design/spec-conformance-testing.md`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let specs_dir = Path::new(&manifest_dir).join("../../specs/design");

    let spec_files = [
        "bytecode-container-format.md",
        "bytecode-instruction-set.md",
    ];

    let mut requirements = BTreeSet::new();

    for filename in &spec_files {
        let path = specs_dir.join(filename);
        println!("cargo:rerun-if-changed={}", path.display());

        if let Ok(content) = fs::read_to_string(&path) {
            extract_requirements(&content, &mut requirements);
        }
    }

    // Also rebuild when the conformance test source changes, so the
    // completeness meta-test stays up to date with include_str!.
    let test_path = Path::new(&manifest_dir).join("src/spec_conformance.rs");
    println!("cargo:rerun-if-changed={}", test_path.display());

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("spec_requirements.rs");

    let mut code = String::from("// Auto-generated from specs/design/*.md — do not edit.\n\n");

    for req in &requirements {
        let ident = req.replace('-', "_");
        code.push_str(&format!(
            "#[allow(dead_code)] pub const {ident}: &str = \"{req}\";\n"
        ));
    }

    code.push('\n');
    code.push_str("pub const ALL: &[&str] = &[\n");
    for req in &requirements {
        code.push_str(&format!("    \"{req}\",\n"));
    }
    code.push_str("];\n");

    fs::write(&dest, code).unwrap();
}

/// Extracts all `**REQ-XX-NNN**` bold markers from markdown content.
fn extract_requirements(content: &str, out: &mut BTreeSet<String>) {
    let mut rest = content;
    while let Some(start) = rest.find("**REQ-") {
        let after_open = &rest[start + 2..]; // skip opening **
        if let Some(end) = after_open.find("**") {
            let id = &after_open[..end];
            // Sanity check: must look like REQ-XX-NNN
            if id.starts_with("REQ-") && id.len() >= 8 {
                out.insert(id.to_string());
            }
            rest = &after_open[end + 2..];
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_requirements() {
        let md = "The header is 256 bytes. **REQ-CF-001** And magic is **REQ-CF-002** here.";
        let mut reqs = BTreeSet::new();
        extract_requirements(md, &mut reqs);
        assert_eq!(reqs.len(), 2);
        assert!(reqs.contains("REQ-CF-001"));
        assert!(reqs.contains("REQ-CF-002"));
    }

    #[test]
    fn test_extract_ignores_non_req_bold() {
        let md = "This is **bold text** and **REQ-CF-010** is a req.";
        let mut reqs = BTreeSet::new();
        extract_requirements(md, &mut reqs);
        assert_eq!(reqs.len(), 1);
        assert!(reqs.contains("REQ-CF-010"));
    }
}
