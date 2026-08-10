//! Project discovery pipeline
//!
//! Detects existing PLC project structures (Beremiz, TwinCAT) in a directory
//! and returns the set of source files to load. When no specific project
//! structure is detected, falls back to enumerating all supported files.
//!
//! The detector chain runs in priority order: Beremiz → TwinCAT → Fallback.
//! The first match wins.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use log::{info, trace};

use crate::file_type::FileType;
use crate::libraries::{LibraryName, LibraryReference};

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

/// The type of PLC project that was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectType {
    /// Beremiz project (plc.xml found in directory)
    Beremiz,
    /// TwinCAT 3 project (.plcproj found in directory)
    TwinCat,
    /// No specific project structure detected; all supported files enumerated
    Unstructured,
}

/// The result of project discovery.
#[derive(Debug)]
pub struct DiscoveredProject {
    /// What kind of project was detected
    pub project_type: ProjectType,
    /// The root directory of the discovered project
    pub root_dir: PathBuf,
    /// The source files to load, in deterministic order
    pub files: Vec<PathBuf>,
    /// The compatibility libraries the project declares it references, in
    /// declaration order and deduplicated by name. Read from a `.plcproj`'s
    /// `<PlaceholderReference>` / `<LibraryReference>` elements
    /// (`REQ-CL-sources-001`); system libraries (`<SystemLibrary>true`) are
    /// skipped. For a TwinCAT project the list additionally carries a
    /// synthetic reference per implicit bundled library (`REQ-CL-sources-008`),
    /// appended after the declared references. Empty for project types that
    /// carry no library references.
    /// Callers resolve these against the bundled registry
    /// ([`crate::libraries::LibraryRegistry::resolve_references`]) to decide
    /// which libraries to activate.
    pub library_references: Vec<LibraryReference>,
    /// Problems found during discovery that should not abort discovery of
    /// the rest of the project -- currently just `.plcproj`
    /// `<Compile Include="...">` entries that don't resolve to a real
    /// file. Discovery still returns all files that DID resolve, but
    /// these are genuine errors: a project that names a file it doesn't
    /// have is broken, and callers must still treat the overall result
    /// as failed (matching the "keep going, but still fail the build"
    /// behavior of e.g. MSBuild's `CS2001`).
    pub errors: Vec<Diagnostic>,
}

/// Discover the project structure in a directory.
///
/// Tries each detector in priority order (Beremiz, TwinCAT) and returns
/// the first match. If no specific project structure is detected, falls
/// back to enumerating all supported files.
///
/// Returns an error if the directory does not exist or cannot be read.
pub fn discover(dir: &Path) -> Result<DiscoveredProject, Diagnostic> {
    info!("Discovering project structure in: {}", dir.display());

    // Validate the directory exists and is readable
    if !dir.is_dir() {
        return Err(Diagnostic::problem(
            Problem::CannotReadDirectory,
            Label::file(
                FileId::from_path(dir),
                format!(
                    "Directory does not exist or is not a directory: {}",
                    dir.display()
                ),
            ),
        ));
    }

    if let Some(project) = detect_beremiz(dir) {
        info!("Detected Beremiz project");
        return Ok(project);
    }

    if let Some(result) = detect_twincat(dir) {
        let project = result?;
        info!(
            "Detected TwinCAT project with {} files",
            project.files.len()
        );
        return Ok(project);
    }

    Ok(detect_fallback(dir))
}

/// Detect a Beremiz project by checking for `plc.xml` in the directory.
///
/// Beremiz projects contain `plc.xml` (PLCopen TC6 XML) and optionally
/// `beremiz.xml` (IDE settings). Only `plc.xml` is loaded.
fn detect_beremiz(dir: &Path) -> Option<DiscoveredProject> {
    let plc_xml = dir.join("plc.xml");
    if plc_xml.is_file() {
        Some(DiscoveredProject {
            project_type: ProjectType::Beremiz,
            root_dir: dir.to_path_buf(),
            files: vec![plc_xml],
            library_references: vec![],
            errors: vec![],
        })
    } else {
        None
    }
}

/// Recursively collects all regular files under `dir`.
///
/// Skips hidden directories (name starts with `.` -- `.git`, `.idea`,
/// `.vs`, etc., all commonly present alongside real TwinCAT checkouts)
/// and does not follow symlinks (treated as neither a file nor a
/// directory), which also rules out symlink cycles. Each directory's
/// entries are sorted by name before recursing, so the result is
/// deterministic regardless of filesystem iteration order.
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }

        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            walk_files(&entry.path(), out);
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
}

/// Detect a TwinCAT project by searching for `.plcproj` files.
///
/// Searches recursively, since real TwinCAT layouts commonly nest
/// `.plcproj` files several levels below the directory a user would
/// naturally point the tool at (e.g. a Visual-Studio-style
/// solution/project structure).
///
/// A real solution commonly has more than one `.plcproj` -- a main PLC
/// project plus one or more library/shared sub-projects that it (or
/// each other) reference types from. All `.plcproj` files found in
/// *different* directories are therefore merged into a single
/// compilation unit, the same principle already applied to LSP
/// workspace folders. Multiple `.plcproj` in the *same* directory are a
/// different, previously-observed case (a stale duplicate/rename
/// artifact, not a second sub-project): only the first (sorted) is kept
/// per directory, preserving the original deterministic-pick behavior
/// for that case.
///
/// Returns `None` if no `.plcproj` exists anywhere. Returns
/// `Some(Err(...))` if a `.plcproj` is found but malformed.
fn detect_twincat(dir: &Path) -> Option<Result<DiscoveredProject, Diagnostic>> {
    let mut files = Vec::new();
    walk_files(dir, &mut files);

    let mut candidates: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| {
            trace!("Check if file {path:?} is plcproj");
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("plcproj"))
        })
        .collect();
    candidates.sort();

    // Keep only the first (sorted) .plcproj per directory -- collapses
    // same-directory duplicates without discarding genuine sub-projects
    // that live in different directories.
    let mut seen_dirs = HashSet::new();
    let mut plcproj_paths: Vec<PathBuf> = Vec::new();
    for path in candidates {
        let dir_key = path.parent().unwrap_or(dir).to_path_buf();
        if seen_dirs.insert(dir_key) {
            plcproj_paths.push(path);
        }
    }

    if plcproj_paths.is_empty() {
        return None;
    }

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
        let project = match parse_plcproj(plcproj_path, plcproj_dir) {
            Ok(project) => project,
            Err(e) => return Some(Err(e)),
        };

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

    Some(Ok(DiscoveredProject {
        project_type: ProjectType::TwinCat,
        root_dir: merged_root_dir,
        files: merged_files,
        library_references: merged_library_references,
        errors: merged_errors,
    }))
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

/// Fallback detection: recursively enumerate all supported files under
/// the directory.
///
/// Returns files sorted alphabetically for deterministic ordering.
fn detect_fallback(dir: &Path) -> DiscoveredProject {
    let mut files = Vec::new();
    walk_files(dir, &mut files);

    let mut files: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| FileType::from_path(path).is_supported())
        .collect();

    files.sort();

    info!("Fallback detection found {} supported files", files.len());

    DiscoveredProject {
        project_type: ProjectType::Unstructured,
        root_dir: dir.to_path_buf(),
        files,
        library_references: vec![],
        errors: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// The references the project itself declared, excluding the synthetic
    /// entries injected for implicit libraries — so assertions about declared
    /// references stay stable as the implicit list changes.
    fn without_implicit(references: &[LibraryReference]) -> Vec<&LibraryReference> {
        references
            .iter()
            .filter(|reference| !TWINCAT_IMPLICIT_LIBRARIES.contains(&reference.name.as_str()))
            .collect()
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
        assert!(names.contains(&"Tc2_BuiltIns"));
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
    fn discover_when_empty_directory_then_returns_unstructured() {
        let dir = TempDir::new().unwrap();
        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.is_empty());
    }

    #[test]
    fn discover_when_unknown_files_then_returns_unstructured_with_empty_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("readme.txt"), "hello").unwrap();
        fs::write(dir.path().join("data.csv"), "a,b,c").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.is_empty());
    }

    // -- Beremiz detection tests --

    #[test]
    fn discover_when_plc_xml_present_then_returns_beremiz() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("plc.xml"), "<project/>").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Beremiz);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_name().unwrap(), "plc.xml");
    }

    #[test]
    fn discover_when_beremiz_with_extra_files_then_loads_only_plc_xml() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("plc.xml"), "<project/>").unwrap();
        fs::write(dir.path().join("beremiz.xml"), "<beremiz/>").unwrap();
        fs::write(dir.path().join("extra.st"), "PROGRAM END_PROGRAM").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Beremiz);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_name().unwrap(), "plc.xml");
    }

    #[test]
    fn detect_beremiz_when_no_plc_xml_then_returns_none() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("other.xml"), "<data/>").unwrap();

        assert!(detect_beremiz(dir.path()).is_none());
    }

    // -- TwinCAT detection tests --

    #[test]
    fn discover_when_plcproj_present_then_returns_twincat() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
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

    // -- TwinCAT library-reference parsing tests --

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

        let declared = without_implicit(&result.library_references);
        assert_eq!(declared.len(), 2);
        assert_eq!(declared[0].name.as_str(), "Tc2_System");
        assert_eq!(declared[0].version.as_deref(), Some("*"));
        assert_eq!(declared[0].namespace.as_deref(), Some("Tc2_System"));
        assert_eq!(declared[1].name.as_str(), "Tc2_Utilities");
        assert_eq!(declared[1].version.as_deref(), Some("3.3.7.0"));
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
        assert!(without_implicit(&result.library_references).is_empty());
    }

    #[test]
    fn discover_when_multiple_plcproj_reference_same_library_then_deduplicated() {
        let dir = TempDir::new().unwrap();

        let a_dir = dir.path().join("ProjectA");
        fs::create_dir_all(&a_dir).unwrap();
        fs::write(a_dir.join("A.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            a_dir.join("ProjectA.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="A.TcPOU" />
    <PlaceholderReference Include="Tc2_System">
      <Namespace>Tc2_System</Namespace>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let b_dir = dir.path().join("ProjectB");
        fs::create_dir_all(&b_dir).unwrap();
        fs::write(b_dir.join("B.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            b_dir.join("ProjectB.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="B.TcPOU" />
    <PlaceholderReference Include="Tc2_System">
      <Namespace>Tc2_System</Namespace>
    </PlaceholderReference>
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();
        let declared = without_implicit(&result.library_references);
        assert_eq!(declared.len(), 1);
        assert_eq!(declared[0].name.as_str(), "Tc2_System");
    }

    #[test]
    fn detect_twincat_when_no_plcproj_then_returns_none() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();

        assert!(detect_twincat(dir.path()).is_none());
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

    // -- Fallback detection tests --

    #[test]
    fn discover_when_st_files_then_returns_unstructured_sorted() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("b_main.st"), "PROGRAM END_PROGRAM").unwrap();
        fs::write(dir.path().join("a_types.st"), "TYPE END_TYPE").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 2);
        // Should be sorted alphabetically
        assert!(result.files[0].ends_with("a_types.st"));
        assert!(result.files[1].ends_with("b_main.st"));
    }

    #[test]
    fn detect_fallback_when_mixed_file_types_then_returns_only_supported() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();
        fs::write(dir.path().join("config.xml"), "<project/>").unwrap();
        fs::write(dir.path().join("readme.txt"), "hello").unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();

        let result = detect_fallback(dir.path());

        assert_eq!(result.project_type, ProjectType::Unstructured);
        // Should include .st, .xml, .TcPOU but not .txt
        assert_eq!(result.files.len(), 3);
    }

    // -- Priority tests --

    #[test]
    fn discover_when_beremiz_and_st_files_then_beremiz_wins() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("plc.xml"), "<project/>").unwrap();
        fs::write(dir.path().join("extra.st"), "PROGRAM END_PROGRAM").unwrap();

        let result = discover(dir.path()).unwrap();
        assert_eq!(result.project_type, ProjectType::Beremiz);
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
    fn detect_fallback_root_dir_is_set_correctly() {
        let dir = TempDir::new().unwrap();
        let result = detect_fallback(dir.path());
        assert_eq!(result.root_dir, dir.path());
    }

    // -- Recursive discovery tests --

    #[test]
    fn discover_when_plcproj_nested_several_levels_then_finds_it() {
        // Matches a real layout found in a private test corpus:
        // TestProject/TestProject/TestProjectRuntime/TestProjectRuntime.plcproj
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Solution").join("Runtime");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            nested.join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
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
    fn discover_when_nested_plcproj_then_root_dir_is_plcproj_directory() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Solution").join("Runtime");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            nested.join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        // root_dir must be where the .plcproj actually lives, not the
        // top-level directory passed to discover() -- otherwise a
        // .plcproj referencing a file in a further subdirectory of its
        // own would resolve against the wrong base.
        assert_eq!(result.root_dir, nested);
    }

    #[test]
    fn discover_when_nested_plcproj_references_file_in_its_own_subdirectory_then_resolves() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Solution").join("Runtime");
        let pous_dir = nested.join("POUs");
        fs::create_dir_all(&pous_dir).unwrap();
        fs::write(pous_dir.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            nested.join("project.plcproj"),
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
        assert!(result.files[0].ends_with("POUs/MAIN.TcPOU"));
    }

    #[test]
    fn discover_when_hidden_directory_contains_plcproj_then_ignored() {
        // .git/.idea-style directories must not be descended into, both
        // for correctness (a decoy .plcproj shouldn't win) and to avoid
        // wastefully/riskily walking into a real .git tree.
        let dir = TempDir::new().unwrap();
        let hidden = dir.path().join(".git");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("decoy.plcproj"), "<Project/>").unwrap();

        let real = dir.path().join("Runtime");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            real.join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
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
    fn discover_when_multiple_plcproj_candidates_then_picks_deterministically() {
        // Matches a real duplicate found in a private test corpus (an
        // apparent stale rename artifact): two .plcproj files in the
        // same directory. Must not error or pick non-deterministically.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            dir.path().join("AAA.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
        fs::write(dir.path().join("ZZZ.plcproj"), "<Project/>").unwrap();

        let result1 = discover(dir.path()).unwrap();
        let result2 = discover(dir.path()).unwrap();

        // Sorted lexicographically: AAA.plcproj wins over ZZZ.plcproj.
        assert_eq!(result1.files.len(), 1);
        assert_eq!(result1.files, result2.files);
    }

    #[test]
    fn discover_when_multiple_plcproj_in_different_directories_then_merges_all() {
        // A solution with a main PLC project and a separate library
        // sub-project -- both must be loaded together so a type declared
        // in one is visible when referenced from the other.
        let dir = TempDir::new().unwrap();

        let main_dir = dir.path().join("Main");
        fs::create_dir_all(&main_dir).unwrap();
        fs::write(main_dir.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            main_dir.join("Main.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let lib_dir = dir.path().join("SharedLib");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("FB_Shared.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            lib_dir.join("SharedLib.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="FB_Shared.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ends_with("MAIN.TcPOU")));
        assert!(result.files.iter().any(|f| f.ends_with("FB_Shared.TcPOU")));
    }

    #[test]
    fn discover_when_multiple_plcproj_in_different_directories_then_root_dir_is_top_level() {
        let dir = TempDir::new().unwrap();

        let main_dir = dir.path().join("Main");
        fs::create_dir_all(&main_dir).unwrap();
        fs::write(main_dir.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            main_dir.join("Main.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MAIN.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let lib_dir = dir.path().join("SharedLib");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("FB_Shared.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            lib_dir.join("SharedLib.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="FB_Shared.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        // With more than one sub-project merged, there is no single
        // meaningful ".plcproj directory" to fall back on -- unlike the
        // single-.plcproj case, root_dir is the top-level directory that
        // was passed to discover().
        assert_eq!(result.root_dir, dir.path());
    }

    #[test]
    fn discover_when_same_file_referenced_by_two_plcproj_then_deduplicated() {
        // Two sub-projects that both reference the same physical file
        // (a shared dependency living in a common directory) must only
        // load and declare it once.
        let dir = TempDir::new().unwrap();

        let shared_dir = dir.path().join("Common");
        fs::create_dir_all(&shared_dir).unwrap();
        fs::write(shared_dir.join("GVL_Shared.TcGVL"), "<TcPlcObject/>").unwrap();

        let a_dir = dir.path().join("ProjectA");
        fs::create_dir_all(&a_dir).unwrap();
        fs::write(
            a_dir.join("ProjectA.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="..\Common\GVL_Shared.TcGVL" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let b_dir = dir.path().join("ProjectB");
        fs::create_dir_all(&b_dir).unwrap();
        fs::write(
            b_dir.join("ProjectB.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="..\Common\GVL_Shared.TcGVL" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let result = discover(dir.path()).unwrap();

        let matches: Vec<_> = result
            .files
            .iter()
            .filter(|f| f.ends_with("GVL_Shared.TcGVL"))
            .collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn detect_fallback_when_files_nested_in_subdirectories_then_finds_them() {
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("src").join("nested");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(dir.path().join("a_top.st"), "PROGRAM END_PROGRAM").unwrap();
        fs::write(subdir.join("b_nested.st"), "PROGRAM END_PROGRAM").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 2);
        assert!(result.files[0].ends_with("a_top.st"));
        assert!(result.files[1].ends_with("nested/b_nested.st"));
    }

    #[test]
    fn detect_fallback_when_hidden_directory_present_then_ignored() {
        let dir = TempDir::new().unwrap();
        let hidden = dir.path().join(".git");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("decoy.st"), "PROGRAM END_PROGRAM").unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();

        let result = detect_fallback(dir.path());

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("main.st"));
    }

    #[cfg(unix)]
    #[test]
    fn detect_fallback_when_symlinked_directory_then_not_followed() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        let real_subdir = dir.path().join("real");
        fs::create_dir_all(&real_subdir).unwrap();
        fs::write(real_subdir.join("main.st"), "PROGRAM END_PROGRAM").unwrap();

        // Symlink pointing back at the parent directory -- if followed,
        // this would recurse infinitely.
        let link = dir.path().join("link_to_self");
        symlink(dir.path(), &link).unwrap();

        let result = detect_fallback(dir.path());

        // Only the real file is found; the symlink is not traversed.
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("real/main.st"));
    }
}
