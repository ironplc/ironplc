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

use ironplc_dsl::bindings::PouBinding;
use ironplc_dsl::common::{Library, LibraryElementKind};
use ironplc_dsl::core::FileId;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;
use tempfile::TempDir;

use crate::discovery::discover;
use crate::libraries::manifest::LibraryManifest;
use crate::libraries::{LibraryName, LibraryReference, LibraryRegistry};
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

/// Write a minimal, valid `Tc2_System`-style library package under `root`.
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
    let loaded = registry.load(&LibraryName::from("Fixture")).unwrap();
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
        "name = \"Tc2_System\"\nvendor = \"Beckhoff Automation GmbH\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n",
        &file_id,
    )
    .unwrap();
    assert_eq!(manifest.name, "Tc2_System");
    assert_eq!(manifest.vendor, "Beckhoff Automation GmbH");
    assert_eq!(manifest.default_version, "1.0.0");

    // Missing `default_version` is rejected.
    let err = LibraryManifest::from_toml(
        "name = \"Tc2_System\"\nvendor = \"Beckhoff Automation GmbH\"\nreferences = [\"https://example.com\"]\n",
        &file_id,
    )
    .unwrap_err();
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

/// REQ-LF-sources-004: The manifest records the public references the library
/// was authored from (a non-empty `references` list).
#[spec_test(REQ_LF_sources_004)]
fn sources_spec_req_lf_004_manifest_records_references() {
    let file_id = FileId::from_string("library.toml");
    let manifest = LibraryManifest::from_toml(
        "name = \"Tc2_System\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com/a\", \"https://example.com/b\"]\n",
        &file_id,
    )
    .unwrap();
    assert_eq!(manifest.references.len(), 2);

    // An empty `references` list is rejected.
    let err = LibraryManifest::from_toml(
        "name = \"Tc2_System\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = []\n",
        &file_id,
    )
    .unwrap_err();
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

/// REQ-LF-sources-005: A manifest may carry a per-version bindings table
/// (quoted version key) mapping a POU name to `{ intrinsic = "<name>" }` or
/// `"declare-only"`.
#[spec_test(REQ_LF_sources_005)]
fn sources_spec_req_lf_005_bindings_table_parses_both_value_forms() {
    let file_id = FileId::from_string("library.toml");
    let manifest = LibraryManifest::from_toml(
        "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n\
         [\"1.0.0\".bindings]\nLTRUNC = { intrinsic = \"trunc_lreal\" }\nLREAL_TO_FMTSTR = \"declare-only\"\n",
        &file_id,
    )
    .unwrap();
    let bindings = manifest.bindings.get("1.0.0").unwrap();
    assert_eq!(
        bindings.get("LTRUNC"),
        Some(&PouBinding::Intrinsic {
            name: "trunc_lreal".to_string()
        })
    );
    assert_eq!(
        bindings.get("LREAL_TO_FMTSTR"),
        Some(&PouBinding::DeclareOnly)
    );
}

/// REQ-LF-sources-006: A malformed bindings entry — including the unquoted
/// dotted version key, which parses as nested tables — is rejected with
/// P6010, and validation covers every version table, not only the default
/// version.
#[spec_test(REQ_LF_sources_006)]
fn sources_spec_req_lf_006_malformed_bindings_rejected_in_any_version() {
    let file_id = FileId::from_string("library.toml");
    let identity = "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n";

    // The unquoted-key trap: `[1.0.0.bindings]` is three nested tables.
    let err = LibraryManifest::from_toml(
        &format!("{identity}[1.0.0.bindings]\nLTRUNC = \"declare-only\"\n"),
        &file_id,
    )
    .unwrap_err();
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());

    // An unknown binding value form is rejected.
    let err = LibraryManifest::from_toml(
        &format!("{identity}[\"1.0.0\".bindings]\nLTRUNC = \"native\"\n"),
        &file_id,
    )
    .unwrap_err();
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());

    // A malformed table for a *non-default* version is still rejected.
    let err = LibraryManifest::from_toml(
        &format!("{identity}[\"2.0.0\".bindings]\nLTRUNC = 42\n"),
        &file_id,
    )
    .unwrap_err();
    assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
}

/// REQ-LF-sources-007: A bound or declare-only POU still appears in its
/// version's `.st` files with its full interface and a body of exactly `;`;
/// the loader exposes the declaration for analysis and the binding in the
/// side-table.
#[spec_test(REQ_LF_sources_007)]
fn sources_spec_req_lf_007_bound_pou_declared_with_empty_body() {
    let dir = TempDir::new().unwrap();
    let version_dir = dir.path().join("Fixture").join("1.0.0");
    fs::create_dir_all(&version_dir).unwrap();
    fs::write(
        dir.path().join("Fixture").join("library.toml"),
        "name = \"Fixture\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n\
         [\"1.0.0\".bindings]\nLTRUNC = { intrinsic = \"trunc_lreal\" }\n",
    )
    .unwrap();
    fs::write(
        version_dir.join("Fixture.st"),
        "FUNCTION LTRUNC : LREAL\nVAR_INPUT\n    IN : LREAL;\nEND_VAR\n;\nEND_FUNCTION\n",
    )
    .unwrap();

    let registry = LibraryRegistry::with_root(dir.path());
    let loaded = registry.load(&LibraryName::from("Fixture")).unwrap();

    // The full interface is declared (so `check` and type resolution work
    // unchanged) with an empty statement list for the `;` body.
    let function = loaded
        .library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::FunctionDeclaration(f) if f.name.original() == "LTRUNC" => Some(f),
            _ => None,
        })
        .expect("bound POU must be declared in the version's .st");
    assert_eq!(function.variables.len(), 1);
    assert!(
        function.body.is_empty(),
        "the `;` body parses to an empty statement list and is never the implementation"
    );

    // The binding rides the side-table, along with the library file set.
    let bound = loaded.bindings.get("LTRUNC").expect("binding recorded");
    assert_eq!(bound.library, "Fixture");
    assert_eq!(
        bound.binding,
        PouBinding::Intrinsic {
            name: "trunc_lreal".to_string()
        }
    );
    let library_file = FileId::from_path(&version_dir.join("Fixture.st"));
    assert!(loaded.bindings.is_library_file(&library_file));
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
    let err = registry.load(&LibraryName::from("Bad")).unwrap_err();
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
    project.set_activated_libraries(vec![LibraryName::from("Tc2_System")]);
    assert_eq!(
        project.activated_libraries(),
        [LibraryName::from("Tc2_System")]
    );

    let (libraries, diagnostics) = project.load_activated_libraries();
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert_eq!(libraries.len(), 1);
    assert!(
        declares_pi(&libraries[0].library),
        "activated Tc2_System must provide the global constant PI"
    );
}

// ---------------------------------------------------------------------------
// Deferred to later phases (wired now so the meta-test passes).
// ---------------------------------------------------------------------------

/// REQ-CL-sources-001: The compiler reads the set of referenced libraries from
/// a discovered `.plcproj` project file's declared library references and
/// activates the matching bundled libraries.
#[spec_test(REQ_CL_sources_001)]
fn sources_spec_req_cl_001_reads_plcproj_library_references() {
    let dir = TempDir::new().unwrap();
    // A POU the project compiles, referenced by <Compile Include>.
    fs::write(
        dir.path().join("main.st"),
        "FUNCTION_BLOCK FB END_FUNCTION_BLOCK",
    )
    .unwrap();
    // A .plcproj declaring both reference shapes plus a skipped system library.
    fs::write(
        dir.path().join("proj.plcproj"),
        r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="main.st" />
    <PlaceholderReference Include="Tc2_System">
      <DefaultResolution>Tc2_System, * (Beckhoff Automation GmbH)</DefaultResolution>
      <Namespace>Tc2_System</Namespace>
    </PlaceholderReference>
    <LibraryReference Include="Tc2_Utilities,3.3.7.0,Beckhoff Automation GmbH">
      <Namespace>Tc2_Utilities</Namespace>
    </LibraryReference>
    <PlaceholderReference Include="VisuElems">
      <SystemLibrary>true</SystemLibrary>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
    )
    .unwrap();

    let discovered = discover(dir.path()).unwrap();

    // Both vendor references are read; the system library is skipped.
    let names: Vec<&str> = discovered
        .library_references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect();
    assert!(names.contains(&"Tc2_System"), "got: {names:?}");
    assert!(names.contains(&"Tc2_Utilities"), "got: {names:?}");
    assert!(
        !names.contains(&"VisuElems"),
        "system library must be skipped, got: {names:?}"
    );

    // The placeholder's Namespace and wildcard version were captured.
    let placeholder = discovered
        .library_references
        .iter()
        .find(|reference| reference.name.as_str() == "Tc2_System")
        .unwrap();
    assert_eq!(placeholder.namespace.as_deref(), Some("Tc2_System"));
    assert_eq!(placeholder.version.as_deref(), Some("*"));

    // The pinned LibraryReference's version was parsed from its Include field.
    let pinned = discovered
        .library_references
        .iter()
        .find(|reference| reference.name.as_str() == "Tc2_Utilities")
        .unwrap();
    assert_eq!(pinned.version.as_deref(), Some("3.3.7.0"));
    assert_eq!(pinned.namespace.as_deref(), Some("Tc2_Utilities"));
}

/// REQ-CL-sources-003: Resolution from a project's library reference to a
/// bundled library is by strict, case-sensitive name match.
#[spec_test(REQ_CL_sources_003)]
fn sources_spec_req_cl_003_reference_matched_by_strict_name() {
    let dir = TempDir::new().unwrap();
    write_library_package(
        dir.path(),
        "Tc2_System",
        "VAR_GLOBAL CONSTANT PI : LREAL := 3.14; END_VAR",
    );
    let registry = LibraryRegistry::with_root(dir.path());
    let declared_in = FileId::from_string("proj.plcproj");

    let reference = |name: &str, version: &str| LibraryReference {
        name: LibraryName::from(name),
        version: Some(version.to_string()),
        namespace: None,
        declared_in: declared_in.clone(),
    };

    // A `*` version resolves to the single bundled version by name alone.
    let (activated, diagnostics) = registry.resolve_references(&[reference("Tc2_System", "*")]);
    assert_eq!(activated, [LibraryName::from("Tc2_System")]);
    assert!(diagnostics.is_empty());

    // A pinned version that differs from the bundled one still resolves: the
    // version is not used to select a package (only the name is).
    let (activated, diagnostics) =
        registry.resolve_references(&[reference("Tc2_System", "9.9.9.9")]);
    assert_eq!(activated, [LibraryName::from("Tc2_System")]);
    assert!(diagnostics.is_empty());

    // A differently-cased name does NOT match -- matching is case-sensitive.
    let (activated, diagnostics) = registry.resolve_references(&[reference("tc2_system", "*")]);
    assert!(activated.is_empty(), "case-insensitive match leaked in");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Problem::LibraryNotFound.code());
}

/// REQ-CL-sources-004: If a project references a library IronPLC does not
/// bundle, the compiler emits a diagnostic that names the missing library.
#[spec_test(REQ_CL_sources_004)]
fn sources_spec_req_cl_004_diagnoses_unshipped_library() {
    let dir = TempDir::new().unwrap();
    write_library_package(
        dir.path(),
        "Tc2_System",
        "VAR_GLOBAL CONSTANT PI : LREAL := 3.14; END_VAR",
    );
    let registry = LibraryRegistry::with_root(dir.path());

    let missing = LibraryReference {
        name: LibraryName::from("Tc3_Module"),
        version: Some("*".to_string()),
        namespace: None,
        declared_in: FileId::from_string("proj.plcproj"),
    };
    let (activated, diagnostics) = registry.resolve_references(&[missing]);

    assert!(activated.is_empty());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Problem::LibraryNotFound.code());
    // The diagnostic names the missing library so the resulting
    // undefined-symbol errors are explained.
    assert!(
        diagnostics[0].primary.message.contains("Tc3_Module"),
        "diagnostic must name the missing library: {}",
        diagnostics[0].primary.message
    );
}

/// REQ-CL-sources-007: The manifest records the public references the library
/// was authored from (a non-empty `references` list), enforced as a provenance
/// conformance test that walks every bundled manifest.
///
/// This is the machine-checkable half of the authoring policy in
/// `specs/steering/compatibility-library-authoring.md`: every bundled library
/// must record a factual, non-empty list of the public references it was
/// authored from. The human-only half (no forbidden input was used, clearance
/// was performed) stays a reviewer responsibility.
#[spec_test(REQ_CL_sources_007)]
fn sources_spec_req_cl_007_provenance_references_recorded() {
    let registry = LibraryRegistry::bundled();
    let names = registry.library_names();

    // There is at least one bundled library to vouch for (guards against the
    // walk silently passing over an empty or misrooted registry).
    assert!(
        !names.is_empty(),
        "expected at least one bundled compatibility library"
    );

    for name in &names {
        // Loading validates the manifest is well-formed (`from_toml` rejects a
        // malformed or field-missing manifest) and, in particular, that its
        // `references` list is non-empty.
        let loaded = registry.load(name).unwrap_or_else(|diagnostic| {
            panic!("bundled library `{name}` must load: {diagnostic:?}")
        });
        assert!(
            !loaded.manifest.references.is_empty(),
            "bundled library `{name}` must record a non-empty `references` list"
        );
    }
}
