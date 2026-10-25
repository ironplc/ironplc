//! Workspace-level guard that a backend depends on the lowered program alone.
//!
//! A backend that can reach analysis or the syntax tree will, at some
//! convenient place, decide again what lowering already decided. Rust lets a
//! crate name only its direct dependencies, so this guard reads each backend's
//! manifest and fails if its `[dependencies]` name a crate a backend never
//! depends on (`specs/steering/backend-guide.md`, "What the crate may depend
//! on"; REQ-LOW-codegen-002 for the bytecode backend).
//!
//! The guard finds the backends itself, so a new backend is checked from its
//! first commit without registering here: a backend is a workspace member
//! whose `[dependencies]` name `ironplc-ir`, other than a crate that builds or
//! drives the pipeline.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ironplc_test::manifest::{dependencies, package_name, workspace_members};

/// The crate that holds the lowered program.
const IR: &str = "ironplc-ir";

/// The crates that depend on `ironplc-ir` without being a backend: they build
/// the lowered program, or pass it to a backend.
const PIPELINE: &[&str] = &[
    // Names the standard function blocks by the lowered program's
    // `StandardFunctionBlock` until lowering generates them.
    "ironplc-analyzer",
    "ironplc-lowering",
    "ironplc-project",
];

/// The crates that hold the syntax tree or analysis, or build the lowered
/// program from them.
const FRONT_END: &[&str] = &[
    "ironplc-dsl",
    "ironplc-parser",
    "ironplc-analyzer",
    "ironplc-lowering",
];

/// The bytecode backend's target: only that backend depends on them.
const BYTECODE_BACKEND: &str = "ironplc-codegen";
const BYTECODE_TARGET: &[&str] = &["ironplc-container", "ironplc-vm"];

/// Each backend among `crates` (package name to `[dependencies]`), with the
/// crates its `[dependencies]` name that a backend never depends on.
fn violations(crates: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let backends: Vec<&String> = crates
        .iter()
        .filter(|(name, deps)| {
            !PIPELINE.contains(&name.as_str()) && deps.iter().any(|dep| dep == IR)
        })
        .map(|(name, _)| name)
        .collect();

    let mut out = Vec::new();
    for backend in &backends {
        for dep in &crates[*backend] {
            let front_end = FRONT_END.contains(&dep.as_str());
            let other_backend = backends.contains(&dep);
            let other_target =
                backend.as_str() != BYTECODE_BACKEND && BYTECODE_TARGET.contains(&dep.as_str());
            if front_end || other_backend || other_target {
                out.push(format!("{backend} depends on {dep}"));
            }
        }
    }
    out
}

/// Each workspace member of `compiler/Cargo.toml`, by package name, with the
/// crates its `[dependencies]` name.
fn workspace_crates(compiler_dir: &Path) -> BTreeMap<String, Vec<String>> {
    let workspace = fs::read_to_string(compiler_dir.join("Cargo.toml")).unwrap();
    workspace_members(&workspace)
        .into_iter()
        .filter_map(|member| {
            let manifest = fs::read_to_string(compiler_dir.join(member).join("Cargo.toml")).ok()?;
            Some((package_name(&manifest)?, dependencies(&manifest)))
        })
        .collect()
}

#[test]
fn every_backend_depends_on_no_front_end_crate() {
    let compiler_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = workspace_crates(&compiler_dir);

    assert!(crates.contains_key(BYTECODE_BACKEND));
    assert_eq!(violations(&crates), Vec::<String>::new());
}

// ---------------------------------------------------------------------------
// Fixture-based unit tests (no dependency on live repo state)
// ---------------------------------------------------------------------------

fn crates(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries
        .iter()
        .map(|(name, deps)| {
            let deps = deps.iter().map(|dep| dep.to_string()).collect();
            (name.to_string(), deps)
        })
        .collect()
}

#[test]
fn violations_when_backend_depends_on_ir_alone_then_none() {
    let crates = crates(&[("ironplc-wasm", &["ironplc-ir", "ironplc-problems"])]);

    assert!(violations(&crates).is_empty());
}

#[test]
fn violations_when_backend_depends_on_analyzer_then_reports_it() {
    let crates = crates(&[("ironplc-wasm", &["ironplc-ir", "ironplc-analyzer"])]);

    assert_eq!(
        violations(&crates),
        vec!["ironplc-wasm depends on ironplc-analyzer"]
    );
}

#[test]
fn violations_when_pipeline_crate_depends_on_front_end_then_none() {
    let crates = crates(&[("ironplc-lowering", &["ironplc-ir", "ironplc-analyzer"])]);

    assert!(violations(&crates).is_empty());
}

#[test]
fn violations_when_crate_does_not_depend_on_ir_then_not_a_backend() {
    let crates = crates(&[("ironplc-plc2plc", &["ironplc-dsl"])]);

    assert!(violations(&crates).is_empty());
}

#[test]
fn violations_when_backend_depends_on_another_backend_then_reports_it() {
    let crates = crates(&[
        ("ironplc-codegen", &["ironplc-ir"]),
        ("ironplc-wasm", &["ironplc-ir", "ironplc-codegen"]),
    ]);

    assert_eq!(
        violations(&crates),
        vec!["ironplc-wasm depends on ironplc-codegen"]
    );
}

#[test]
fn violations_when_bytecode_target_then_only_bytecode_backend_may_depend_on_it() {
    let crates = crates(&[
        ("ironplc-codegen", &["ironplc-ir", "ironplc-container"]),
        ("ironplc-wasm", &["ironplc-ir", "ironplc-vm"]),
    ]);

    assert_eq!(
        violations(&crates),
        vec!["ironplc-wasm depends on ironplc-vm"]
    );
}
