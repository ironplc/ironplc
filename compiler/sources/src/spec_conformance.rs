//! Spec conformance tests for compatibility libraries (sources-owned
//! requirements): the on-disk package format and the loader/activation set.
//!
//! Each test is annotated with `#[spec_test(REQ_CL_sources_NNN)]` or
//! `#[spec_test(REQ_LF_sources_NNN)]`, which adds `#[test]` and references a
//! build-script-generated constant so the test fails to compile if the
//! requirement is removed from the spec. The `all_spec_requirements_have_tests`
//! meta-test asserts every sources-owned requirement has a test here.
//!
//! Requirements whose implementation lands in a later phase are wired with
//! `#[ignore]` so the meta-test still passes.
//!
//! See `specs/design/compatibility-libraries.md` and
//! `specs/design/compatibility-library-format.md`.

use std::fs;

use ironplc_dsl::common::{Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;
use tempfile::TempDir;

use crate::libraries::manifest::LibraryManifest;
use crate::libraries::LibraryRegistry;
use crate::SourceProject;

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

/// True when `library` declares a global constant named `PI`.
fn declares_pi(library: &Library) -> bool {
    library.elements.iter().any(|element| {
        matches!(element, LibraryElementKind::GlobalVarDeclarations(globals)
            if globals.iter().any(|v| v.identifier.symbolic_id().is_some_and(|id| id.original() == "PI")))
    })
}

/// Write a minimal, valid `Tc2_Math`-style library package under `root`.
fn write_library_package(root: &std::path::Path, name: &str, body: &str) {
    let version_dir = root.join(name).join("1.0.0");
    fs::create_dir_all(&version_dir).unwrap();
    fs::write(
        root.join(name).join("library.toml"),
        format!(
            "name = \"{name}\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n"
        ),
    )
    .unwrap();
    fs::write(version_dir.join(format!("{name}.st")), body).unwrap();
}

// ---------------------------------------------------------------------------
// On-disk format (REQ-LF-sources-*)
// ---------------------------------------------------------------------------

/// REQ-LF-sources-001: A compatibility library is a directory named for the
/// library, containing a `library.toml` manifest and one subdirectory per
/// version, each holding that version's `.st` declaration files.
#[spec_test(REQ_LF_sources_001)]
fn sources_spec_req_lf_001_package_layout_is_read() {
    let dir = TempDir::new().unwrap();
    write_library_package(
        dir.path(),
        "Fixture",
        "VAR_GLOBAL CONSTANT PI : LREAL := 3.14; END_VAR",
    );

    let registry = LibraryRegistry::with_root(dir.path());
    let loaded = registry.load("Fixture").expect("package loads");
    // The declaration in the version subdirectory's `.st` file was read.
    assert!(declares_pi(&loaded.library));
}

/// REQ-LF-sources-002: The manifest declares the library's identity — `name`,
/// `vendor`, `default_version` — and the loader rejects a manifest missing any
/// required field.
#[spec_test(REQ_LF_sources_002)]
fn sources_spec_req_lf_002_manifest_declares_identity() {
    let file_id = FileId::from_string("library.toml");
    let manifest = LibraryManifest::from_toml(
        "name = \"Tc2_Math\"\nvendor = \"Beckhoff Automation GmbH\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n",
        &file_id,
    )
    .expect("valid manifest parses");
    assert_eq!(manifest.name, "Tc2_Math");
    assert_eq!(manifest.vendor, "Beckhoff Automation GmbH");
    assert_eq!(manifest.default_version, "1.0.0");

    // Missing `default_version` is rejected.
    let err = LibraryManifest::from_toml(
        "name = \"Tc2_Math\"\nvendor = \"Beckhoff Automation GmbH\"\nreferences = [\"https://example.com\"]\n",
        &file_id,
    )
    .expect_err("missing field is rejected");
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

/// REQ-LF-sources-004: The manifest records the public references the library
/// was authored from (a non-empty `references` list).
#[spec_test(REQ_LF_sources_004)]
fn sources_spec_req_lf_004_manifest_records_references() {
    let file_id = FileId::from_string("library.toml");
    let manifest = LibraryManifest::from_toml(
        "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com/a\", \"https://example.com/b\"]\n",
        &file_id,
    )
    .expect("valid manifest parses");
    assert_eq!(manifest.references.len(), 2);

    // An empty `references` list is rejected.
    let err = LibraryManifest::from_toml(
        "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = []\n",
        &file_id,
    )
    .expect_err("empty references is rejected");
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

// ---------------------------------------------------------------------------
// Loader / activation behavior (REQ-CL-sources-*)
// ---------------------------------------------------------------------------

/// REQ-CL-sources-002: A library's manifest declares its identity — `name`,
/// `vendor`, `default_version` — and the loader rejects a manifest missing any
/// required field.
#[spec_test(REQ_CL_sources_002)]
fn sources_spec_req_cl_002_loader_validates_manifest_identity() {
    let dir = TempDir::new().unwrap();
    let lib_dir = dir.path().join("Bad");
    fs::create_dir_all(lib_dir.join("1.0.0")).unwrap();
    // Manifest missing `default_version` and `references`.
    fs::write(
        lib_dir.join("library.toml"),
        "name = \"Bad\"\nvendor = \"ACME\"\n",
    )
    .unwrap();

    let registry = LibraryRegistry::with_root(dir.path());
    let err = registry
        .load("Bad")
        .expect_err("invalid manifest is rejected on load");
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

/// REQ-CL-sources-005: The active library set derives only from explicit
/// activation; the compiler never infers a library from POU source content.
#[spec_test(REQ_CL_sources_005)]
fn sources_spec_req_cl_005_active_set_not_inferred_from_source() {
    let mut project = SourceProject::new();
    // Source that *uses* PI must not auto-activate any library.
    project.add_source(
        FileId::from_string("main.st"),
        "FUNCTION_BLOCK FB VAR x : LREAL := PI; END_VAR END_FUNCTION_BLOCK".to_string(),
    );

    assert!(
        project.activated_libraries().is_empty(),
        "no library may be inferred from source content"
    );
    let (libraries, diagnostics) = project.load_activated_libraries();
    assert!(libraries.is_empty());
    assert!(diagnostics.is_empty());
}

/// REQ-CL-sources-006: An explicit activation request (e.g. the `--library
/// <name>` CLI option) activates the named bundled library for source that has
/// no project context.
#[spec_test(REQ_CL_sources_006)]
fn sources_spec_req_cl_006_explicit_activation_activates_library() {
    let mut project = SourceProject::new();
    project.set_activated_libraries(vec!["Tc2_Math".to_string()]);
    assert_eq!(project.activated_libraries(), ["Tc2_Math"]);

    let (libraries, diagnostics) = project.load_activated_libraries();
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert_eq!(libraries.len(), 1);
    assert!(
        declares_pi(&libraries[0]),
        "activated Tc2_Math must provide the global constant PI"
    );
}

// ---------------------------------------------------------------------------
// Deferred to later phases (wired now so the meta-test passes).
// ---------------------------------------------------------------------------

/// REQ-CL-sources-001: The compiler reads the set of referenced libraries from
/// a discovered `.plcproj` project file's declared library references and
/// activates the matching bundled libraries.
#[spec_test(REQ_CL_sources_001)]
#[ignore = "phase 2: reading library references from .plcproj"]
fn sources_spec_req_cl_001_reads_plcproj_library_references() {}

/// REQ-CL-sources-003: Resolution from a project's library reference to a
/// bundled library is by strict, case-sensitive name match.
#[spec_test(REQ_CL_sources_003)]
#[ignore = "phase 2: project reference -> bundled library name matching"]
fn sources_spec_req_cl_003_reference_matched_by_strict_name() {}

/// REQ-CL-sources-004: If a project references a library IronPLC does not
/// bundle, the compiler emits a diagnostic that names the missing library.
#[spec_test(REQ_CL_sources_004)]
#[ignore = "phase 2: diagnose a referenced-but-unshipped library"]
fn sources_spec_req_cl_004_diagnoses_unshipped_library() {}

/// REQ-CL-sources-007: The manifest records the public references the library
/// was authored from (a non-empty `references` list), enforced as a provenance
/// conformance test that walks every bundled manifest.
#[spec_test(REQ_CL_sources_007)]
#[ignore = "phase 4: provenance walk over every bundled manifest"]
fn sources_spec_req_cl_007_provenance_references_recorded() {}
