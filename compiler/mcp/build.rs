//! Build script that generates:
//!
//! 1. `spec_requirements.rs` from requirement markers in the MCP server design
//!    spec (`**REQ-STL-mcp-001**`, `**REQ-TOL-mcp-010**`, `**REQ-ARC-mcp-001**`).
//!
//! 2. `problem_docs.rs` — a lookup function that maps problem codes (e.g.
//!    `"P0001"`) to their embedded `.rst` documentation and CSV title, used by
//!    the `explain_diagnostic` tool (REQ-TOL-mcp-072).
//!
//! See `specs/design/spec-conformance-testing.md`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    generate_problem_docs(&manifest_dir);
    ironplc_spec_requirements_gen::generate(&["mcp-server.md"]);
}

// ---------------------------------------------------------------------------
// Problem-doc embedding (REQ-TOL-mcp-072)
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
