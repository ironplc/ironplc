//! `.plcproj` discovery, parsing, and merging.
//!
//! A `.plcproj` is the actual compilation-unit manifest: it lists the
//! source files (`<Compile Include="...">`) and compatibility-library
//! references (`<PlaceholderReference>`/`<LibraryReference>`) that make
//! up one TwinCAT PLC project. `super::sln` resolves *which* `.plcproj`
//! files belong to a solution; this module does the actual XML parsing
//! and, when a solution has more than one `.plcproj`, merges them into a
//! single compilation unit.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use super::{DiscoveredProject, ProjectType};
use crate::libraries::{LibraryName, LibraryReference};
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// Bundled libraries TwinCAT provides to every PLC project without a
/// reference anywhere in the `.plcproj` — the built-in (compiler-operator)
/// surface. Discovering a TwinCAT project always activates these
/// (`REQ-CL-sources-008`); there is no way to opt out, because there is no
/// TwinCAT project without them.
///
/// Deliberately a hard-coded list for now: a manifest-driven "implicit"
/// marker would need to express *which vendor's* project format implies the
/// library, and that mechanism does not exist yet. When a second vendor
/// project discovery arrives, replace this with the real mechanism.
const TWINCAT_IMPLICIT_LIBRARIES: &[&str] = &["Tc2_BuiltIns"];

/// Parse and merge a resolved, non-empty list of `.plcproj` paths into
/// one compilation unit.
///
/// A real solution commonly has more than one `.plcproj` -- a main PLC
/// project plus one or more library/shared sub-projects that it (or
/// each other) reference types from. All given `.plcproj` files are
/// merged into a single compilation unit, the same principle already
/// applied to LSP workspace folders.
pub(super) fn merge_plcproj_projects(
    dir: &Path,
    plcproj_paths: Vec<PathBuf>,
) -> Result<DiscoveredProject, Diagnostic> {
    // <Compile Include="..."> paths in a .plcproj are always relative to
    // that .plcproj file's own directory, not the (possibly higher, now
    // that files can be nested arbitrarily deep) directory originally
    // passed to discover() -- each is parsed against its own directory
    // regardless of how many sub-projects are being merged.
    let single = plcproj_paths.len() == 1;
    let mut merged_files = Vec::new();
    let mut merged_errors = Vec::new();
    let mut merged_library_references: Vec<LibraryReference> = Vec::new();
    let mut seen_files = HashSet::new();
    let mut seen_libraries: HashSet<LibraryName> = HashSet::new();
    let mut merged_root_dir = dir.to_path_buf();

    for plcproj_path in &plcproj_paths {
        let plcproj_dir = plcproj_path.parent().unwrap_or(dir);
        let project = parse_plcproj(plcproj_path, plcproj_dir)?;

        if single {
            merged_root_dir = project.root_dir.clone();
        }

        // A library referenced by more than one sub-project must only be
        // activated once. Dedup by name (first reference wins), matching the
        // name-only resolution the registry performs downstream.
        for reference in project.library_references {
            if seen_libraries.insert(reference.name.clone()) {
                merged_library_references.push(reference);
            }
        }

        for file in project.files {
            // A file referenced by more than one sub-project (a shared
            // dependency) must only be loaded/declared once. Dedup by
            // canonical path, not the raw resolved path -- two
            // sub-projects in different directories that both reach the
            // same file via a relative `..` segment resolve to distinct,
            // non-canonicalized paths that still name the same file.
            let key = fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
            if seen_files.insert(key) {
                merged_files.push(file);
            }
        }
        merged_errors.extend(project.errors);
    }

    // Implicit (vendor built-in) libraries: TwinCAT provides these names to
    // every project with no reference anywhere in the .plcproj, so discovering
    // a TwinCAT project activates them (`REQ-CL-sources-008`). The synthetic
    // reference joins the merged list before downstream resolution and is
    // deduped against any real reference of the same name.
    append_implicit_references(
        &mut merged_library_references,
        &mut seen_libraries,
        &plcproj_paths[0],
    );

    Ok(DiscoveredProject {
        project_type: ProjectType::TwinCat,
        root_dir: merged_root_dir,
        files: merged_files,
        library_references: merged_library_references,
        errors: merged_errors,
    })
}

/// Append a synthetic reference for every library in
/// [`TWINCAT_IMPLICIT_LIBRARIES`] the project does not already reference
/// (`REQ-CL-sources-008`).
///
/// TwinCAT provides implicit libraries to every project, so the discovered
/// project file itself is the activation signal; `declared_in` anchors any
/// downstream diagnostic on that project file.
fn append_implicit_references(
    references: &mut Vec<LibraryReference>,
    seen: &mut HashSet<LibraryName>,
    declared_in: &Path,
) {
    for name in TWINCAT_IMPLICIT_LIBRARIES {
        let name = LibraryName::from(*name);
        if seen.insert(name.clone()) {
            references.push(LibraryReference {
                name,
                version: None,
                namespace: None,
                declared_in: FileId::from_path(declared_in),
            });
        }
    }
}

/// Parse a `.plcproj` file and extract `<Compile Include="...">` paths.
fn parse_plcproj(plcproj_path: &Path, root_dir: &Path) -> Result<DiscoveredProject, Diagnostic> {
    let content = fs::read_to_string(plcproj_path).map_err(|e| {
        Diagnostic::problem(
            Problem::CannotReadFile,
            Label::file(
                FileId::from_path(plcproj_path),
                format!("Cannot read .plcproj file: {e}"),
            ),
        )
    })?;

    let doc = roxmltree::Document::parse(&content).map_err(|e| {
        Diagnostic::problem(
            Problem::XmlMalformed,
            Label::file(
                FileId::from_path(plcproj_path),
                format!("Malformed .plcproj XML: {e}"),
            ),
        )
    })?;

    let mut files = Vec::new();
    let mut errors = Vec::new();
    let library_references = parse_library_references(&doc, plcproj_path);

    // Find all <Compile Include="..."> elements anywhere in the document.
    // An entry that doesn't resolve to a real file (a stale reference, a
    // case-sensitivity mismatch, a genuinely missing asset) is recorded
    // as an error and skipped -- but does not abort the whole project:
    // every other per-file problem in the codebase already works this
    // way, and one bad reference shouldn't hide every other, perfectly
    // valid file in the same project from ever being checked. The
    // command as a whole must still fail, though (see `errors` field doc).
    for node in doc.descendants() {
        if node.is_element() && node.tag_name().name() == "Compile" {
            if let Some(include) = node.attribute("Include") {
                // Resolve relative to the .plcproj directory, normalizing
                // Windows-style backslash separators
                let normalized = include.replace('\\', "/");
                let resolved = root_dir.join(&normalized);

                if !resolved.is_file() {
                    errors.push(Diagnostic::problem(
                        Problem::CannotReadFile,
                        Label::file(
                            FileId::from_path(plcproj_path),
                            format!(
                                "Referenced file does not exist: {} (resolved to {})",
                                include,
                                resolved.display()
                            ),
                        ),
                    ));
                    continue;
                }

                files.push(resolved);
            }
        }
    }

    Ok(DiscoveredProject {
        project_type: ProjectType::TwinCat,
        root_dir: root_dir.to_path_buf(),
        files,
        library_references,
        errors,
    })
}

/// Extract the compatibility-library references from a parsed `.plcproj`.
///
/// A `.plcproj` states which libraries the project uses (the vendor's own
/// record) in `<ItemGroup>` as one of two element types (`REQ-CL-sources-001`):
///
/// - `<PlaceholderReference Include="Name">` — the common, version-flexible
///   form. The version lives in `<DefaultResolution>Name, Version (Vendor)`,
///   usually the `*` wildcard.
/// - `<LibraryReference Include="Name,Version,Vendor">` — a concrete, pinned
///   reference.
///
/// Both may carry a `<Namespace>` child (the qualifier the source may write).
/// A reference marked `<SystemLibrary>true</SystemLibrary>` (CODESYS /
/// visualization system libraries such as `VisuElems`) is not vendor-authored
/// ST we bundle, so the first increment skips it. Element names are matched by
/// local name, so the MSBuild default `xmlns` does not need special handling —
/// the same way `<Compile>` is already read.
fn parse_library_references(
    doc: &roxmltree::Document,
    plcproj_path: &Path,
) -> Vec<LibraryReference> {
    let declared_in = FileId::from_path(plcproj_path);
    let mut references = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        let (raw_name, version) = match node.tag_name().name() {
            "PlaceholderReference" => {
                let Some(include) = node.attribute("Include") else {
                    continue;
                };
                // The version, when present, is the middle field of
                // `<DefaultResolution>Name, Version (Vendor)</DefaultResolution>`.
                let version = child_text(node, "DefaultResolution")
                    .and_then(|resolution| default_resolution_version(&resolution));
                (include.to_string(), version)
            }
            "LibraryReference" => {
                let Some(include) = node.attribute("Include") else {
                    continue;
                };
                // Include is `Name,Version,Vendor`.
                let mut fields = include.splitn(3, ',');
                let name = fields.next().unwrap_or("").trim().to_string();
                let version = fields
                    .next()
                    .map(|field| field.trim().to_string())
                    .filter(|field| !field.is_empty());
                (name, version)
            }
            _ => continue,
        };

        // Skip system libraries for now (REQ-CL-sources-001 note).
        if child_text(node, "SystemLibrary")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
        {
            continue;
        }

        let name = raw_name.trim();
        if name.is_empty() {
            continue;
        }

        let namespace = child_text(node, "Namespace")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        references.push(LibraryReference {
            name: LibraryName::from(name),
            version,
            namespace,
            declared_in: declared_in.clone(),
        });
    }

    references
}

/// The trimmed text of the first direct child element named `tag`, if any.
fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == tag)
        .and_then(|child| child.text())
        .map(str::to_string)
}

/// Extract the version from a `<DefaultResolution>` value of the form
/// `Name, Version (Vendor)`. Returns `None` when no version is present.
fn default_resolution_version(resolution: &str) -> Option<String> {
    let after_name = resolution.split_once(',')?.1;
    let version = after_name.split('(').next()?.trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::super::discover;
    use super::super::fixtures::{write_file, write_plcproj};
    use super::*;
    use tempfile::TempDir;

    /// A TwinCAT project naming two PLC sub-projects. Written literally
    /// rather than generated -- see `fixtures.rs` for why.
    const TSPROJ_NAMING_TWO_PLCPROJ: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TcSmProject ProjectGUID="{9406D69C-EBA9-4591-A513-578A75D14426}">
  <Project>
    <Plc>
      <Project GUID="{6DADE760-7FAC-4830-92BA-478C8595D673}" Name="Main" PrjFilePath="Main\Main.plcproj" AmsPort="851" />
      <Project GUID="{1F2E3D4C-5B6A-4978-8899-AABBCCDDEEFF}" Name="SharedLib" PrjFilePath="SharedLib\SharedLib.plcproj" AmsPort="852" />
    </Plc>
  </Project>
</TcSmProject>
"#;

    /// A TwinCAT project naming `ProjectA` and `ProjectB`, the two
    /// sub-projects the merge tests share a dependency between.
    const TSPROJ_NAMING_PROJECT_A_AND_B: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TcSmProject ProjectGUID="{9406D69C-EBA9-4591-A513-578A75D14426}">
  <Project>
    <Plc>
      <Project GUID="{6DADE760-7FAC-4830-92BA-478C8595D673}" Name="ProjectA" PrjFilePath="ProjectA\ProjectA.plcproj" AmsPort="851" />
      <Project GUID="{1F2E3D4C-5B6A-4978-8899-AABBCCDDEEFF}" Name="ProjectB" PrjFilePath="ProjectB\ProjectB.plcproj" AmsPort="852" />
    </Plc>
  </Project>
</TcSmProject>
"#;

    /// A TwinCAT project naming one PLC sub-project, `Runtime.plcproj`.
    const TSPROJ_NAMING_RUNTIME_PLCPROJ: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TcSmProject ProjectGUID="{9406D69C-EBA9-4591-A513-578A75D14426}">
  <Project>
    <Plc>
      <Project GUID="{6DADE760-7FAC-4830-92BA-478C8595D673}" Name="Runtime" PrjFilePath="Runtime\Runtime.plcproj" AmsPort="851" />
    </Plc>
  </Project>
</TcSmProject>
"#;

    /// A `.plcproj` whose only `<Compile>` entry reaches a file in a
    /// sibling directory via `..`.
    const PLCPROJ_SHARING_COMMON_GVL: &str = r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="..\Common\GVL_Shared.TcGVL" />
  </ItemGroup>
</Project>
"#;

    #[test]
    fn append_implicit_references_when_already_seen_then_appends_nothing() {
        let mut references = Vec::new();
        let mut seen: HashSet<LibraryName> = TWINCAT_IMPLICIT_LIBRARIES
            .iter()
            .map(|name| LibraryName::from(*name))
            .collect();

        append_implicit_references(&mut references, &mut seen, Path::new("project.plcproj"));

        assert!(references.is_empty());
    }

    #[test]
    fn discover_when_plcproj_has_no_references_then_implicit_libraries_added() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        let names: Vec<&str> = result
            .library_references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect();
        assert_eq!(names, ["Tc2_BuiltIns"]);
    }

    #[test]
    fn discover_when_plcproj_already_references_implicit_library_then_not_duplicated() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
    <PlaceholderReference Include="Tc2_BuiltIns">
      <Namespace>Tc2_BuiltIns</Namespace>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        let count = result
            .library_references
            .iter()
            .filter(|reference| reference.name.as_str() == "Tc2_BuiltIns")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn discover_when_plcproj_with_subdirectory_paths_then_resolves() {
        let dir = TempDir::new().unwrap();
        let pous_dir = dir.path().join("POUs");
        fs::create_dir(&pous_dir).unwrap();
        fs::write(pous_dir.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="POUs\MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("MAIN.TcPOU"));
    }

    #[test]
    fn discover_when_plcproj_references_missing_file_then_returns_error_but_keeps_discovering() {
        // A single unresolvable <Compile> entry must not abort discovery
        // for the whole project -- it's recorded as an error and
        // skipped, matching how every other per-file problem in the
        // codebase is handled. It must still be surfaced as an error,
        // though (not downgraded to a mere warning): the caller is
        // responsible for still failing the overall command.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MISSING.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].primary.message.contains("MISSING.TcPOU"));
    }

    #[test]
    fn discover_when_plcproj_has_valid_and_missing_entries_then_valid_file_still_resolves() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("A.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="A.TcPOU" />
    <Compile Include="MISSING.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("A.TcPOU"));
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].primary.message.contains("MISSING.TcPOU"));
    }

    #[test]
    fn discover_when_plcproj_has_library_references_then_reads_them() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
    <PlaceholderReference Include="Tc2_System">
      <DefaultResolution>Tc2_System, * (Beckhoff Automation GmbH)</DefaultResolution>
      <Namespace>Tc2_System</Namespace>
    </PlaceholderReference>
    <LibraryReference Include="Tc2_Utilities,3.3.7.0,Beckhoff Automation GmbH">
      <Namespace>Tc2_Utilities</Namespace>
    </LibraryReference>
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        // The two declared references in declaration order, then the implicit
        // Tc2_BuiltIns every TwinCAT project gets.
        let references = &result.library_references;
        assert_eq!(references.len(), 3);
        assert_eq!(references[0].name.as_str(), "Tc2_System");
        assert_eq!(references[0].version.as_deref(), Some("*"));
        assert_eq!(references[0].namespace.as_deref(), Some("Tc2_System"));
        assert_eq!(references[1].name.as_str(), "Tc2_Utilities");
        assert_eq!(references[1].version.as_deref(), Some("3.3.7.0"));
        assert_eq!(references[2].name.as_str(), "Tc2_BuiltIns");
        assert_eq!(references[2].version, None);
    }

    #[test]
    fn discover_when_plcproj_reference_is_system_library_then_skipped() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <PlaceholderReference Include="VisuElems">
      <SystemLibrary>true</SystemLibrary>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        // The system library is skipped; only the implicit Tc2_BuiltIns
        // every TwinCAT project gets remains.
        let names: Vec<&str> = result
            .library_references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect();
        assert_eq!(names, ["Tc2_BuiltIns"]);
    }

    #[test]
    fn discover_when_multiple_plcproj_reference_same_library_then_deduplicated() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_PROJECT_A_AND_B,
        );

        for (name, source) in [("ProjectA", "A.TcPOU"), ("ProjectB", "B.TcPOU")] {
            let project_dir = dir.path().join(name);
            write_file(&project_dir.join(source), "<TcPlcObject/>");
            write_file(
                &project_dir.join(format!("{name}.plcproj")),
                &format!(
                    r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="{source}" />
    <PlaceholderReference Include="Tc2_System">
      <Namespace>Tc2_System</Namespace>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#
                ),
            );
        }

        let result = discover(dir.path()).unwrap();

        // The shared reference is deduplicated to one entry, followed by the
        // implicit Tc2_BuiltIns every TwinCAT project gets.
        let names: Vec<&str> = result
            .library_references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect();
        assert_eq!(names, ["Tc2_System", "Tc2_BuiltIns"]);
    }

    #[test]
    fn discover_when_plcproj_with_multiple_files_then_preserves_order() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("B_Second.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(dir.path().join("A_First.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(dir.path().join("C_Third.TcDUT"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="B_Second.TcPOU" />
    <Compile Include="A_First.TcPOU" />
    <Compile Include="C_Third.TcDUT" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 3);
        // Order should match .plcproj order, not alphabetical
        assert!(result.files[0].ends_with("B_Second.TcPOU"));
        assert!(result.files[1].ends_with("A_First.TcPOU"));
        assert!(result.files[2].ends_with("C_Third.TcDUT"));
    }

    #[test]
    fn discover_when_plcproj_with_malformed_xml_then_returns_diagnostic() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            "THIS IS NOT VALID XML <><>",
        )
        .unwrap();

        let result = discover(dir.path());
        assert!(result.is_err());

        let diag = result.unwrap_err();
        assert_eq!(diag.code, "P0006"); // XmlMalformed
    }

    #[test]
    fn discover_when_twincat_and_plcproj_error_propagates() {
        // A .plcproj that references a missing file must not abort
        // discovery through the detect_twincat -> discover path -- the
        // error is collected on `DiscoveredProject::errors`, not
        // returned as `Err`, so the rest of the project can still be
        // enumerated. Callers must still surface it as a failure.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="DOES_NOT_EXIST.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn discover_when_all_plcproj_entries_unresolvable_then_returns_empty_with_errors() {
        // Not a special case -- matches the existing "no <Compile>
        // entries at all" precedent (empty files list, no `Err`), but
        // still reports one error per unresolvable entry.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MISSING_A.TcPOU" />
    <Compile Include="MISSING_B.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn discover_when_plcproj_with_no_compile_entries_then_returns_empty_twincat() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <PropertyGroup>
    <Name>EmptyProject</Name>
  </PropertyGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();
        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert!(result.files.is_empty());
    }

    #[test]
    fn discover_when_tsproj_names_plcproj_in_different_directories_then_merges_all() {
        // A solution with a main PLC project and a separate library
        // sub-project -- both must be loaded together so a type declared
        // in one is visible when referenced from the other.
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("Main.tsproj"), TSPROJ_NAMING_TWO_PLCPROJ);
        write_plcproj(
            &dir.path().join("Main").join("Main.plcproj"),
            &["MAIN.TcPOU"],
        );
        write_plcproj(
            &dir.path().join("SharedLib").join("SharedLib.plcproj"),
            &["FB_Shared.TcPOU"],
        );

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ends_with("MAIN.TcPOU")));
        assert!(result.files.iter().any(|f| f.ends_with("FB_Shared.TcPOU")));
    }

    #[test]
    fn discover_when_multiple_plcproj_merged_then_root_dir_is_manifest_directory() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("Main.tsproj"), TSPROJ_NAMING_TWO_PLCPROJ);
        write_plcproj(
            &dir.path().join("Main").join("Main.plcproj"),
            &["MAIN.TcPOU"],
        );
        write_plcproj(
            &dir.path().join("SharedLib").join("SharedLib.plcproj"),
            &["FB_Shared.TcPOU"],
        );

        let result = discover(dir.path()).unwrap();

        // With more than one sub-project merged, there is no single
        // meaningful ".plcproj directory" to fall back on -- unlike the
        // single-.plcproj case, root_dir is the directory holding the
        // manifest that named them.
        assert_eq!(result.root_dir, dir.path());
    }

    #[test]
    fn discover_when_single_plcproj_named_by_manifest_then_root_dir_is_plcproj_directory() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        let plcproj_dir = dir.path().join("Runtime");
        write_plcproj(&plcproj_dir.join("Runtime.plcproj"), &["MAIN.TcPOU"]);

        let result = discover(dir.path()).unwrap();

        // root_dir must be where the .plcproj actually lives, not the
        // directory holding the manifest that named it -- otherwise a
        // .plcproj referencing a file in a further subdirectory of its
        // own would resolve against the wrong base.
        assert_eq!(result.root_dir, plcproj_dir);
    }

    #[test]
    fn discover_when_plcproj_references_file_in_its_own_subdirectory_then_resolves() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        write_plcproj(
            &dir.path().join("Runtime").join("Runtime.plcproj"),
            &["POUs\\MAIN.TcPOU"],
        );

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("POUs/MAIN.TcPOU"));
    }

    #[test]
    fn discover_when_same_file_referenced_by_two_plcproj_then_deduplicated() {
        // Two sub-projects that both reference the same physical file
        // (a shared dependency living in a common directory) must only
        // load and declare it once.
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_PROJECT_A_AND_B,
        );
        write_file(
            &dir.path().join("Common").join("GVL_Shared.TcGVL"),
            "<TcPlcObject/>",
        );
        write_file(
            &dir.path().join("ProjectA").join("ProjectA.plcproj"),
            PLCPROJ_SHARING_COMMON_GVL,
        );
        write_file(
            &dir.path().join("ProjectB").join("ProjectB.plcproj"),
            PLCPROJ_SHARING_COMMON_GVL,
        );

        let result = discover(dir.path()).unwrap();

        let matches: Vec<_> = result
            .files
            .iter()
            .filter(|f| f.ends_with("GVL_Shared.TcGVL"))
            .collect();
        assert_eq!(matches.len(), 1);
    }
}
