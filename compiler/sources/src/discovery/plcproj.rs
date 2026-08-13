//! `.plcproj` discovery, parsing, and merging.
//!
//! A `.plcproj` is the actual compilation-unit manifest: it lists the
//! source files (`<Compile Include="...">`) and compatibility-library
//! references (`<PlaceholderReference>`/`<LibraryReference>`) that make
//! up one TwinCAT PLC project. `super::sln` resolves *which* `.plcproj`
//! files belong to a solution (or falls back to [`collect_plcproj_via_walk`]
//! when there's no `.sln`); this module does the actual XML parsing and,
//! when a solution has more than one `.plcproj`, merges them into a
//! single compilation unit.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use log::trace;

use super::{walk_files, DiscoveredProject, ProjectType};
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

/// Recursively collect `.plcproj` candidates and keep only the first
/// (sorted) per directory -- collapses same-directory duplicates (a
/// previously observed stale-rename artifact) without discarding
/// genuine sub-projects that live in different directories.
///
/// Searches recursively, since real TwinCAT layouts commonly nest
/// `.plcproj` files several levels below the directory a user would
/// naturally point the tool at.
pub(super) fn collect_plcproj_via_walk(dir: &Path) -> Vec<PathBuf> {
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

    let mut seen_dirs = HashSet::new();
    let mut plcproj_paths: Vec<PathBuf> = Vec::new();
    for path in candidates {
        let dir_key = path.parent().unwrap_or(dir).to_path_buf();
        if seen_dirs.insert(dir_key) {
            plcproj_paths.push(path);
        }
    }
    plcproj_paths
}

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
    use super::*;

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
}
