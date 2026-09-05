//! Project discovery pipeline
//!
//! Detects existing PLC project structures (TwinCAT, Beremiz) from a path
//! and returns the set of source files to load. The path is either the
//! project manifest itself or the folder holding it -- both say the same
//! thing, so an editor that can only open a folder is as well served as a
//! command line that can name a file.
//!
//! What a path is taken to mean, in order; the first match wins:
//!
//! 1. a `.sln` or `.plcproj` file -> TwinCAT
//! 2. a directory holding exactly one `.sln` or `.plcproj` -> TwinCAT
//! 3. a `plc.xml` file, or a directory holding one -> Beremiz
//! 4. anything else -> unstructured
//!
//! Rules 2 and 3 read the given directory and nothing below it. A project
//! is defined by its manifest, and the manifest names everything else by
//! reference, so the nesting a real layout has is traversed by reference
//! rather than by search: a `.sln` names its `.tsproj` files, a `.tsproj`
//! names its `.plcproj` files, and a `.plcproj` names its sources.
//! Searching a tree instead would have to guess when it turned up more
//! than one candidate, and guessing is how a stale project file left
//! behind by a rename gets compiled instead of the live one.
//!
//! Recursion happens in exactly one place: [`detect_fallback`], rule 4,
//! where no manifest format is in play at all and enumeration *is* the
//! project definition. A directory that is ambiguous under rule 2 (two
//! `.plcproj` files, say) is simply not a TwinCAT project and falls
//! through to that same enumeration -- there is nothing to guess between,
//! because nothing is being selected.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use log::info;

use crate::file_type::FileType;
use crate::libraries::LibraryReference;

#[cfg(test)]
mod fixtures;
mod plcproj;
mod sln;
use plcproj::merge_plcproj_projects;
use sln::{find_manifests, resolve_plcproj_via_sln};

/// The manifests that name a TwinCAT project: the two files a user
/// actually opens.
///
/// `.tsproj` is deliberately absent. It is part of the resolution chain --
/// a `.sln` reaches its `.plcproj` files through one -- but it is not an
/// entry point, because nobody opens a solution by naming its system
/// project.
const TWINCAT_MANIFESTS: &[&str] = &["sln", "plcproj"];

/// The file a Beremiz project is named by.
const BEREMIZ_MANIFEST: &str = "plc.xml";

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

/// The outcome of running one detector against a directory.
///
/// The distinction that matters is between "there is no project of this
/// kind here" and "there is one, but it could not be resolved". Only the
/// former may fall through to the next detector: falling through on the
/// latter would answer an unresolvable manifest with a guess.
enum Detection {
    /// No project of this kind here. Try the next detector.
    NotDetected,
    /// Detected and fully resolved.
    Detected(Box<DiscoveredProject>),
    /// Detected but unresolvable: an unreadable `.sln`, a malformed
    /// `.tsproj`, a chain naming no `.plcproj`. Authoritative -- never
    /// falls through to another detector, because a manifest that was
    /// found and could not be followed is a broken project, not an
    /// absent one, and answering it with an enumeration would be a guess.
    Failed(Diagnostic),
}

/// Discover the project structure at a path.
///
/// `path` is either the project manifest itself or the folder holding it;
/// both say the same thing, and an editor that can only open folders is
/// as well served as a command line that can name a file. Tries each
/// detector in order (TwinCAT, Beremiz) and returns the first match. If
/// the path names no project, falls back to enumerating supported files.
///
/// Returns an error if `path` does not exist, or if a detector found a
/// manifest it could not resolve.
pub fn discover(path: &Path) -> Result<DiscoveredProject, Diagnostic> {
    info!("Discovering project structure at: {}", path.display());

    if !path.exists() {
        return Err(Diagnostic::problem(
            Problem::CannotReadDirectory,
            Label::file(
                FileId::from_path(path),
                format!("Path does not exist: {}", path.display()),
            ),
        ));
    }

    for detect in [detect_twincat, detect_beremiz] {
        match detect(path) {
            Detection::Detected(project) => {
                info!(
                    "Detected {:?} project with {} files",
                    project.project_type,
                    project.files.len()
                );
                return Ok(*project);
            }
            Detection::Failed(diagnostic) => return Err(diagnostic),
            Detection::NotDetected => {}
        }
    }

    Ok(detect_fallback(path))
}

/// Detect a Beremiz project from a `plc.xml`, named either directly or by
/// the folder holding it.
///
/// Beremiz projects contain `plc.xml` (PLCopen TC6 XML) and optionally
/// `beremiz.xml` (IDE settings). Only `plc.xml` is loaded -- which is the
/// point of detecting at all, since enumerating the folder instead would
/// also pick up `beremiz.xml`, an IDE settings file that is not a PLCopen
/// document.
fn detect_beremiz(path: &Path) -> Detection {
    let plc_xml = if path.is_dir() {
        path.join(BEREMIZ_MANIFEST)
    } else if path
        .file_name()
        .is_some_and(|name| name == BEREMIZ_MANIFEST)
    {
        path.to_path_buf()
    } else {
        return Detection::NotDetected;
    };

    if !plc_xml.is_file() {
        return Detection::NotDetected;
    }

    Detection::Detected(Box::new(DiscoveredProject {
        project_type: ProjectType::Beremiz,
        root_dir: plc_xml.parent().unwrap_or(path).to_path_buf(),
        files: vec![plc_xml],
        library_references: vec![],
        errors: vec![],
    }))
}

/// Recursively collects all regular files under `dir`.
///
/// Skips hidden directories (name starts with `.` -- `.git`, `.idea`,
/// `.vs`, etc., all commonly present alongside real TwinCAT checkouts)
/// and does not follow symlinks (treated as neither a file nor a
/// directory), which also rules out symlink cycles. Each directory's
/// entries are sorted by name before recursing, so the result is
/// deterministic regardless of filesystem iteration order.
pub(super) fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
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

/// Detect a TwinCAT project from a `.sln` or `.plcproj`, named either
/// directly or by the folder holding exactly one.
///
/// The manifest found is authoritative and is resolved through the
/// `.sln` -> `.tsproj` -> `PrjFilePath` chain TcXaeShell itself uses. A
/// folder holding more than one manifest names no single project, so it
/// is not a TwinCAT project at all and falls through -- the user can
/// always say which they meant by naming it, and nothing is guessed in
/// the meantime.
///
/// Only the given folder is read. Manifests below it are not searched
/// for: see the module docs.
fn detect_twincat(path: &Path) -> Detection {
    let manifest = if path.is_dir() {
        let manifests = find_manifests(path, TWINCAT_MANIFESTS);
        if manifests.len() != 1 {
            return Detection::NotDetected;
        }
        manifests.into_iter().next().unwrap_or_default()
    } else if is_twincat_manifest(path) {
        path.to_path_buf()
    } else {
        return Detection::NotDetected;
    };

    match resolve_manifest(&manifest) {
        Ok(project) => Detection::Detected(Box::new(project)),
        Err(diagnostic) => Detection::Failed(diagnostic),
    }
}

/// Whether `path` names a TwinCAT project manifest.
fn is_twincat_manifest(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        TWINCAT_MANIFESTS
            .iter()
            .any(|manifest| extension.eq_ignore_ascii_case(manifest))
    })
}

/// Resolve one `.sln` or `.plcproj` into a project.
///
/// A `.sln` reaches its `.plcproj` files through the `.tsproj` files it
/// lists; a `.plcproj` is already the compilation unit.
fn resolve_manifest(manifest_path: &Path) -> Result<DiscoveredProject, Diagnostic> {
    let manifest_dir = manifest_path.parent().unwrap_or(manifest_path);

    let plcproj_paths = if manifest_path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sln"))
    {
        resolve_plcproj_via_sln(manifest_path)?
    } else {
        vec![manifest_path.to_path_buf()]
    };

    merge_plcproj_projects(manifest_dir, plcproj_paths)
}

/// Fallback detection: enumerate the supported files at `path`.
///
/// For a directory that means every supported file beneath it, sorted --
/// the one place discovery recurses, and the one place it may, because
/// with no manifest in play the enumeration *is* the project definition.
/// For a file it means that file, whatever it is: the caller named it, so
/// letting the parser reject it says more than silently dropping it here.
fn detect_fallback(path: &Path) -> DiscoveredProject {
    let files = if path.is_dir() {
        let mut found = Vec::new();
        walk_files(path, &mut found);
        found.retain(|path| FileType::from_path(path).is_supported());
        found.sort();
        found
    } else {
        vec![path.to_path_buf()]
    };

    info!("Fallback detection found {} supported files", files.len());

    DiscoveredProject {
        project_type: ProjectType::Unstructured,
        root_dir: if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        },
        files,
        library_references: vec![],
        errors: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{tree, tree_file};
    use super::*;

    // Every case points at a checked-in tree under
    // `resources/test/discovery`; see `fixtures.rs`.

    // -- Rule 4: unstructured --

    #[test]
    fn discover_when_empty_directory_then_returns_unstructured() {
        // The tree holds only a `.gitkeep`, which `walk_files` skips
        // along with every other dot-entry -- so discovery sees an empty
        // directory, which git cannot track on its own.
        let result = discover(&tree("empty")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.is_empty());
    }

    #[test]
    fn discover_when_unknown_files_then_returns_unstructured_with_empty_files() {
        let result = discover(&tree("unsupported_files")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.is_empty());
    }

    #[test]
    fn discover_when_st_files_then_returns_unstructured_sorted() {
        let result = discover(&tree("loose_st_files")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 2);
        assert!(result.files[0].ends_with("a_types.st"));
        assert!(result.files[1].ends_with("b_main.st"));
    }

    #[test]
    fn detect_fallback_when_mixed_file_types_then_returns_only_supported() {
        let result = detect_fallback(&tree("mixed_file_types"));

        assert_eq!(result.project_type, ProjectType::Unstructured);
        // .st, .xml and .TcPOU are supported; readme.txt is not.
        assert_eq!(result.files.len(), 3);
        assert!(!result.files.iter().any(|f| f.ends_with("readme.txt")));
    }

    #[test]
    fn detect_fallback_root_dir_is_set_correctly() {
        let result = detect_fallback(&tree("empty"));

        assert_eq!(result.root_dir, tree("empty"));
    }

    #[test]
    fn detect_fallback_when_files_nested_in_subdirectories_then_finds_them() {
        let result = discover(&tree("nested_sources")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 2);
        assert!(result.files[0].ends_with("a_top.st"));
        assert!(result.files[1].ends_with("nested/b_nested.st"));
    }

    #[test]
    fn detect_fallback_when_hidden_directory_present_then_ignored() {
        let result = detect_fallback(&tree("hidden_directory"));

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("main.st"));
    }

    #[test]
    fn discover_when_hidden_directory_contains_plcproj_then_ignored() {
        // A decoy `.plcproj` inside a dot-directory must not make the
        // folder a TwinCAT project, nor be enumerated as a source.
        let result = discover(&tree("hidden_directory")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("main.st"));
    }

    #[test]
    fn discover_when_source_file_named_directly_then_returns_that_file() {
        let result = discover(&tree_file("loose_st_files", "b_main.st")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("b_main.st"));
    }

    #[test]
    fn discover_when_path_does_not_exist_then_errors() {
        let error = discover(&tree("no_such_tree")).unwrap_err();

        assert_eq!(error.code, "P6003");
    }

    // -- Rule 3: Beremiz --

    #[test]
    fn discover_when_plc_xml_present_then_returns_beremiz() {
        let result = discover(&tree("beremiz")).unwrap();

        assert_eq!(result.project_type, ProjectType::Beremiz);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_name().unwrap(), "plc.xml");
    }

    #[test]
    fn discover_when_beremiz_with_extra_files_then_loads_only_plc_xml() {
        // The tree also holds beremiz.xml (IDE settings, not a PLCopen
        // document) and extra.st. Enumerating the folder would feed
        // beremiz.xml to the XML parser; detecting the project does not.
        let result = discover(&tree("beremiz")).unwrap();

        assert_eq!(result.project_type, ProjectType::Beremiz);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_name().unwrap(), "plc.xml");
    }

    #[test]
    fn discover_when_plc_xml_named_directly_then_returns_beremiz() {
        let result = discover(&tree_file("beremiz", "plc.xml")).unwrap();

        assert_eq!(result.project_type, ProjectType::Beremiz);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].file_name().unwrap(), "plc.xml");
    }

    #[test]
    fn detect_beremiz_when_no_plc_xml_then_not_detected() {
        assert!(matches!(
            detect_beremiz(&tree("xml_without_plc_xml")),
            Detection::NotDetected
        ));
    }

    // -- Rule 1: the manifest named directly --

    #[test]
    fn discover_when_sln_named_directly_then_resolves_chain() {
        let result = discover(&tree_file("sln_chain", "Solution.sln")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("LIVE.TcPOU"));
    }

    #[test]
    fn discover_when_plcproj_named_directly_then_resolves_it() {
        let result = discover(&tree_file("plcproj_only", "project.plcproj")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("MAIN.TcPOU"));
    }

    #[test]
    fn discover_when_tsproj_named_directly_then_not_a_project() {
        // A .tsproj is part of the chain but never an entry point, so
        // naming one is just naming a file.
        let result = discover(&tree_file("tsproj_only", "Main.tsproj")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    // -- Rule 2: the folder holding exactly one manifest --

    #[test]
    fn discover_when_folder_holds_one_sln_then_resolves_chain() {
        // The case that makes an editor work by default: VS Code opens
        // the folder, not the .sln inside it.
        let result = discover(&tree("sln_chain")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("LIVE.TcPOU"));
    }

    #[test]
    fn discover_when_folder_holds_one_plcproj_then_returns_twincat() {
        let result = discover(&tree("plcproj_only")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("MAIN.TcPOU"));
    }

    #[test]
    fn discover_when_folder_holds_only_tsproj_then_not_a_project() {
        let result = discover(&tree("tsproj_only")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    #[test]
    fn detect_twincat_when_no_manifest_then_not_detected() {
        assert!(matches!(
            detect_twincat(&tree("loose_st_files")),
            Detection::NotDetected
        ));
    }

    #[test]
    fn discover_when_multiple_sln_in_directory_then_not_a_project() {
        // Two solutions name no single project, so the folder is not a
        // TwinCAT project. Nothing is guessed between them; a user who
        // meant one of them says so by naming it.
        let result = discover(&tree("two_sln")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    #[test]
    fn discover_when_multiple_plcproj_in_directory_then_not_a_project() {
        // The real duplicate found in a private test corpus: two
        // .plcproj files in one directory, one a stale rename artifact.
        // Nothing says which is live, so neither is chosen.
        let result = discover(&tree("two_plcproj")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    #[test]
    fn discover_when_sln_and_plcproj_in_directory_then_not_a_project() {
        // Rule 2 counts .sln and .plcproj together: one of each still
        // names no single project.
        let result = discover(&tree("sln_and_plcproj")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    #[test]
    fn discover_when_ambiguous_plcproj_then_naming_one_resolves_it() {
        // The escape hatch: name the manifest instead of the folder.
        let result = discover(&tree_file("two_plcproj", "ZZZ.plcproj")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("Z.TcPOU"));
    }

    // -- Manifests below the opened directory --

    #[test]
    fn discover_when_manifest_only_nested_then_unstructured() {
        // Matches a real layout found in a private test corpus:
        // TestProject/TestProject/TestProjectRuntime/*.plcproj. The tree
        // above the project is not searched, so nothing is chosen from
        // among the manifests below it.
        let result = discover(&tree("nested_manifest")).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
    }

    #[test]
    fn discover_when_manifest_directory_opened_then_resolves() {
        // Opening the folder that holds the manifest is the convention;
        // rule 2 makes it work without naming the file.
        let result = discover(&tree_file("nested_manifest", "Solution/Runtime")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
    }

    // -- A manifest that was found but cannot be followed --

    #[test]
    fn discover_when_sln_names_no_plcproj_then_reports_unresolvable() {
        // Authoritative and broken: reporting beats falling back to an
        // enumeration that ignores what the manifest says.
        let error = discover(&tree("sln_names_no_plcproj")).unwrap_err();

        assert_eq!(error.code, "P6012");
    }

    #[cfg(unix)]
    #[test]
    fn detect_fallback_when_symlinked_directory_then_not_followed() {
        use std::os::unix::fs::symlink;

        // The one case that cannot be a checked-in tree: a symlink
        // pointing at its own parent would make every tool that walks
        // the repository recurse forever, so it is created for the
        // duration of the test and removed with the temporary directory.
        let dir = tempfile::TempDir::new().unwrap();
        let real_subdir = dir.path().join("real");
        std::fs::create_dir_all(&real_subdir).unwrap();
        std::fs::copy(
            tree_file("loose_st_files", "b_main.st"),
            real_subdir.join("main.st"),
        )
        .unwrap();
        symlink(dir.path(), dir.path().join("link_to_self")).unwrap();

        let result = detect_fallback(dir.path());

        // Only the real file is found; the symlink is not traversed.
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("real/main.st"));
    }
}
