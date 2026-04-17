//! Build script that generates:
//!
//! 1. `spec_requirements.rs` from requirement markers in the MCP server design
//!    spec (`**REQ-STL-001**`, `**REQ-TOL-010**`, `**REQ-ARC-001**`).
//!
//! 2. `problem_docs.rs` — a lookup function that maps problem codes (e.g.
//!    `"P0001"`) to their embedded `.rst` documentation and CSV title, used by
//!    the `explain_diagnostic` tool (REQ-TOL-072).
//!
//! See `specs/design/spec-conformance-testing.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    generate_problem_docs(&manifest_dir);
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

// ---------------------------------------------------------------------------
// Problem-doc embedding (REQ-TOL-072)
// ---------------------------------------------------------------------------

/// Generates `problem_docs.rs` which embeds every `P####.rst` file at compile
/// time and provides a lookup function returning `(rst_content, title)`.
fn generate_problem_docs(manifest_dir: &str) {
    let problems_dir = Path::new(manifest_dir).join("../../docs/reference/compiler/problems");
    let csv_path = Path::new(manifest_dir).join("../problems/resources/problem-codes.csv");

    println!("cargo:rerun-if-changed={}", problems_dir.display());
    println!("cargo:rerun-if-changed={}", csv_path.display());

    // Read CSV to build code→title map.
    let mut titles: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(csv_content) = fs::read_to_string(&csv_path) {
        for line in csv_content.lines().skip(1) {
            // Format: Code,Name,Message
            let fields: Vec<&str> = line.splitn(3, ',').collect();
            if fields.len() == 3 {
                titles.insert(fields[0].to_string(), fields[2].to_string());
            }
        }
    }

    // Collect P####.rst files, sorted by code.
    let mut entries: BTreeMap<String, PathBuf> = BTreeMap::new();
    if let Ok(dir) = fs::read_dir(&problems_dir) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('P') && name.ends_with(".rst") {
                let code = name.trim_end_matches(".rst").to_string();
                let abs_path = fs::canonicalize(entry.path()).unwrap();
                println!("cargo:rerun-if-changed={}", abs_path.display());
                entries.insert(code, abs_path);
            }
        }
    }

    // Generate the lookup function.
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("problem_docs.rs");

    let mut code =
        String::from("// Auto-generated from docs/reference/compiler/problems/ — do not edit.\n\n");

    code.push_str("/// Returns `(rst_content, title)` for a known problem code, or `None`.\n");
    code.push_str(
        "pub fn lookup_problem_doc(code: &str) -> Option<(&'static str, &'static str)> {\n",
    );
    code.push_str("    match code {\n");

    for (problem_code, abs_path) in &entries {
        let title = titles.get(problem_code).map(|s| s.as_str()).unwrap_or("");
        // Escape backslashes in the absolute path for the include_str! macro.
        let path_str = abs_path.display().to_string().replace('\\', "/");
        let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
        code.push_str(&format!(
            "        \"{problem_code}\" => Some((include_str!(\"{path_str}\"), \"{escaped_title}\")),\n"
        ));
    }

    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    fs::write(&dest, code).unwrap();
}
