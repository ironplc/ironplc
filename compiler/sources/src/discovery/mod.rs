//! Project discovery pipeline
//!
//! Detects existing PLC project structures (Beremiz, TwinCAT) in a directory
//! and returns the set of source files to load. When no specific project
//! structure is detected, falls back to enumerating all supported files.
//!
//! The detector chain runs in priority order: Beremiz → TwinCAT → Fallback.
//! The first match wins.
//!
//! A project detector reads the directory it was given and nothing below
//! it: a project is defined by its manifest, and the manifest names
//! everything else by reference. The convention that follows from that is
//! **open the folder containing the manifest** -- pointing the tool higher
//! up a tree and searching for manifests would have to guess between
//! candidates, and guessing is how a stale project file left behind by a
//! rename gets compiled instead of the live one. The one detector that
//! does read the whole subtree is [`detect_fallback`], where there is no
//! manifest format in play at all and enumeration *is* the project
//! definition.

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
use sln::{find_manifests, resolve_plcproj_via_sln, resolve_plcproj_via_tsproj};

/// The manifest kinds a TwinCAT project can be opened through, in
/// priority order: a solution names the projects it contains, so it wins
/// over any individual project file sitting in the same directory.
const TWINCAT_MANIFEST_TIERS: &[&str] = &["sln", "tsproj", "plcproj"];

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
    /// No manifest of this kind here. Try the next detector.
    NotDetected,
    /// No manifest here, but one exists deeper in the tree. Still not
    /// this detector's project, so the chain continues -- the hint is
    /// surfaced only if nothing else claims the directory. `manifest` is
    /// the path the hint names, which is how [`discover`] decides that.
    NotDetectedWithHint {
        manifest: PathBuf,
        diagnostic: Diagnostic,
    },
    /// Detected and fully resolved.
    Detected(Box<DiscoveredProject>),
    /// Detected but unresolvable: ambiguous manifests, an unreadable
    /// `.sln`, a malformed `.tsproj`. Authoritative -- never falls
    /// through to another detector.
    Failed(Diagnostic),
}

/// Discover the project structure in a directory.
///
/// Tries each detector in priority order (Beremiz, TwinCAT) and returns
/// the first match. If no specific project structure is detected, falls
/// back to enumerating all supported files.
///
/// Returns an error if the directory does not exist or cannot be read,
/// or if a detector found a project manifest it could not resolve.
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

    let mut hints = Vec::new();
    for detect in [detect_beremiz, detect_twincat] {
        match detect(dir) {
            Detection::Detected(project) => {
                info!(
                    "Detected {:?} project with {} files",
                    project.project_type,
                    project.files.len()
                );
                return Ok(*project);
            }
            Detection::Failed(diagnostic) => return Err(diagnostic),
            Detection::NotDetectedWithHint {
                manifest,
                diagnostic,
            } => hints.push((manifest, diagnostic)),
            Detection::NotDetected => {}
        }
    }

    // A hint says "no project here, but there is one below". That is
    // worth saying only when the directory holds nothing of its own: a
    // directory of loose source files that happens to contain an
    // unrelated solution in some subfolder is not a mistake, and telling
    // its owner to go open the subfolder would be wrong.
    //
    // "Nothing of its own" is measured against the manifest's directory
    // rather than by an empty enumeration, because a real project's
    // sources sit *beside* its manifest and are themselves supported file
    // types. Enumerating those and calling the directory unstructured
    // would compile the project's files while ignoring the manifest that
    // says which of them belong -- the same guess the manifest exists to
    // settle.
    let fallback = detect_fallback(dir);
    match hints.into_iter().next() {
        Some((manifest, diagnostic))
            if fallback
                .files
                .iter()
                .all(|file| file.starts_with(manifest.parent().unwrap_or(&manifest))) =>
        {
            Err(diagnostic)
        }
        _ => Ok(fallback),
    }
}

/// Detect a Beremiz project by checking for `plc.xml` in the directory.
///
/// Beremiz projects contain `plc.xml` (PLCopen TC6 XML) and optionally
/// `beremiz.xml` (IDE settings). Only `plc.xml` is loaded.
fn detect_beremiz(dir: &Path) -> Detection {
    let plc_xml = dir.join("plc.xml");
    if plc_xml.is_file() {
        Detection::Detected(Box::new(DiscoveredProject {
            project_type: ProjectType::Beremiz,
            root_dir: dir.to_path_buf(),
            files: vec![plc_xml],
            library_references: vec![],
            errors: vec![],
        }))
    } else {
        Detection::NotDetected
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

/// Detect a TwinCAT project from the manifest directly in `dir`.
///
/// [`TWINCAT_MANIFEST_TIERS`] are tried in order and the first tier with
/// any manifest present decides the outcome -- a solution folder that
/// also holds a stray `.plcproj` still resolves through its `.sln`.
/// Exactly one manifest at that tier is authoritative and is resolved
/// through the `.sln` -> `.tsproj` -> `PrjFilePath` chain TcXaeShell
/// itself uses; more than one is ambiguous and the user is asked to name
/// the one they meant, rather than having it picked for them by sort
/// order.
///
/// Only `dir` itself is read. If it holds no manifest but one exists
/// below it, that is reported as a hint naming it -- the user pointed at
/// the wrong level, which is a different mistake from having no TwinCAT
/// project at all.
fn detect_twincat(dir: &Path) -> Detection {
    for tier in TWINCAT_MANIFEST_TIERS {
        let manifests = find_manifests(dir, tier);
        match manifests.len() {
            0 => continue,
            1 => {
                return match resolve_manifest(&manifests[0]) {
                    Ok(project) => Detection::Detected(Box::new(project)),
                    Err(diagnostic) => Detection::Failed(diagnostic),
                }
            }
            _ => return Detection::Failed(ambiguous_manifests(dir, tier, &manifests)),
        }
    }

    match find_nested_manifest(dir) {
        Some(nested) => Detection::NotDetectedWithHint {
            diagnostic: manifest_not_in_directory(dir, &nested),
            manifest: nested,
        },
        None => Detection::NotDetected,
    }
}

/// Whether `path` names a project manifest [`discover_from_manifest`] can
/// resolve. Lets a caller that accepts file arguments route a manifest to
/// discovery instead of loading it as source text.
pub fn is_manifest(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        TWINCAT_MANIFEST_TIERS
            .iter()
            .any(|tier| extension.eq_ignore_ascii_case(tier))
    })
}

/// Resolve a project manifest named directly, rather than found by
/// opening the directory that contains it.
///
/// A manifest given by name is authoritative in exactly the way one found
/// in a directory is -- this is how a user resolves the ambiguity of a
/// directory holding several. Returns an error if `path` is not a
/// manifest ([`is_manifest`]) or does not resolve to a project.
pub fn discover_from_manifest(path: &Path) -> Result<DiscoveredProject, Diagnostic> {
    if !is_manifest(path) {
        return Err(sln::unresolvable(path, "not a TwinCAT project manifest"));
    }
    resolve_manifest(path)
}

/// Resolve one manifest of any tier into a project.
fn resolve_manifest(manifest_path: &Path) -> Result<DiscoveredProject, Diagnostic> {
    let manifest_dir = manifest_path.parent().unwrap_or(manifest_path);
    let extension = manifest_path.extension().unwrap_or_default();

    let plcproj_paths = if extension.eq_ignore_ascii_case("sln") {
        resolve_plcproj_via_sln(manifest_path)?
    } else if extension.eq_ignore_ascii_case("tsproj") {
        let plcproj_paths = resolve_plcproj_via_tsproj(manifest_path)?;
        if plcproj_paths.is_empty() {
            return Err(sln::unresolvable(
                manifest_path,
                "the project does not name any TwinCAT PLC project (.plcproj)",
            ));
        }
        plcproj_paths
    } else {
        vec![manifest_path.to_path_buf()]
    };

    merge_plcproj_projects(manifest_dir, plcproj_paths)
}

/// The first manifest of any tier below `dir`, if there is one.
///
/// Used only to make the "you opened the wrong folder" diagnostic name a
/// concrete path; it never selects sources.
fn find_nested_manifest(dir: &Path) -> Option<PathBuf> {
    let mut files = Vec::new();
    walk_files(dir, &mut files);
    files.sort();
    files.into_iter().find(|path| is_manifest(path))
}

/// More than one project manifest of the same tier sits in `dir`, so
/// there is no way to tell which one the user meant.
fn ambiguous_manifests(dir: &Path, tier: &str, manifests: &[PathBuf]) -> Diagnostic {
    let names: Vec<String> = manifests
        .iter()
        .map(|path| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    Diagnostic::problem(
        Problem::ProjectManifestAmbiguous,
        Label::file(
            FileId::from_path(dir),
            format!(
                "{} contains {} .{tier} files ({}); name the one to open instead of the directory",
                dir.display(),
                manifests.len(),
                names.join(", ")
            ),
        ),
    )
}

/// `dir` holds no manifest, but one exists below it -- the user opened
/// the tree above their project rather than the project itself.
fn manifest_not_in_directory(dir: &Path, nested: &Path) -> Diagnostic {
    Diagnostic::problem(
        Problem::ProjectManifestNotInDirectory,
        Label::file(
            FileId::from_path(dir),
            format!(
                "{} contains no project manifest, but {} is one; open the folder containing it",
                dir.display(),
                nested.display()
            ),
        ),
    )
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
    use super::fixtures::{write_file, write_plcproj};
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A solution naming `Main.tsproj`. Written literally rather than
    /// generated: these tests are about which manifest a directory
    /// resolves through, and a generator agreeing with the parser by
    /// construction would hide the parser drifting from the real format.
    /// `sln.rs` exercises the format itself against literal text.
    const SOLUTION_NAMING_MAIN_TSPROJ: &str = r#"Microsoft Visual Studio Solution File, Format Version 12.00
# TcXaeShell Solution File, Format Version 11.00
Project("{B1E792BE-AA5F-4E3C-8C82-674BF9C0715B}") = "Main", "Main.tsproj", "{9406D69C-EBA9-4591-A513-578A75D14426}"
EndProject
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

    /// A TwinCAT project with no PLC part at all -- authoritative, but
    /// nothing for the compiler to build.
    const TSPROJ_NAMING_NOTHING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TcSmProject ProjectGUID="{9406D69C-EBA9-4591-A513-578A75D14426}">
  <Project>
    <Io />
  </Project>
</TcSmProject>
"#;

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
    fn detect_beremiz_when_no_plc_xml_then_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("other.xml"), "<data/>").unwrap();

        assert!(matches!(detect_beremiz(dir.path()), Detection::NotDetected));
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

    // -- TwinCAT library-reference parsing tests --

    #[test]
    fn detect_twincat_when_no_manifest_then_not_detected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();

        assert!(matches!(detect_twincat(dir.path()), Detection::NotDetected));
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
    fn detect_fallback_root_dir_is_set_correctly() {
        let dir = TempDir::new().unwrap();
        let result = detect_fallback(dir.path());
        assert_eq!(result.root_dir, dir.path());
    }

    // -- Manifest tier tests --

    #[test]
    fn discover_when_sln_and_plcproj_in_same_directory_then_sln_wins() {
        // A solution folder that also holds a stray .plcproj resolves
        // through the .sln: the solution says which projects are live.
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Solution.sln"),
            SOLUTION_NAMING_MAIN_TSPROJ,
        );
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        write_plcproj(
            &dir.path().join("Runtime").join("Runtime.plcproj"),
            &["LIVE.TcPOU"],
        );
        write_plcproj(&dir.path().join("Stray.plcproj"), &["STRAY.TcPOU"]);

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("LIVE.TcPOU"));
    }

    #[test]
    fn discover_when_tsproj_and_plcproj_in_same_directory_then_tsproj_wins() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        write_plcproj(
            &dir.path().join("Runtime").join("Runtime.plcproj"),
            &["LIVE.TcPOU"],
        );
        write_plcproj(&dir.path().join("Stray.plcproj"), &["STRAY.TcPOU"]);

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("LIVE.TcPOU"));
    }

    #[test]
    fn discover_when_multiple_sln_in_directory_then_reports_ambiguous() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("A.sln"), SOLUTION_NAMING_MAIN_TSPROJ);
        write_file(&dir.path().join("B.sln"), SOLUTION_NAMING_MAIN_TSPROJ);

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6012");
    }

    #[test]
    fn discover_when_multiple_plcproj_in_directory_then_reports_ambiguous() {
        // The real duplicate found in a private test corpus: two
        // .plcproj files in one directory, one of them a stale rename
        // artifact. Nothing in the directory says which is live, so the
        // user is asked rather than having sort order decide.
        let dir = TempDir::new().unwrap();
        write_plcproj(&dir.path().join("AAA.plcproj"), &["MAIN.TcPOU"]);
        write_plcproj(&dir.path().join("ZZZ.plcproj"), &["MAIN.TcPOU"]);

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6012");
        // The message names both candidates so the user can pick one.
        let description = format!("{error:?}");
        assert!(description.contains("AAA.plcproj"), "{description}");
        assert!(description.contains("ZZZ.plcproj"), "{description}");
    }

    #[test]
    fn discover_when_ambiguous_plcproj_then_naming_one_resolves_it() {
        // The escape hatch the ambiguity diagnostic points at: name the
        // manifest instead of the directory that holds several.
        let dir = TempDir::new().unwrap();
        write_plcproj(&dir.path().join("AAA.plcproj"), &["A.TcPOU"]);
        write_plcproj(&dir.path().join("ZZZ.plcproj"), &["Z.TcPOU"]);

        let result = discover_from_manifest(&dir.path().join("ZZZ.plcproj")).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("Z.TcPOU"));
    }

    // -- Manifests below the opened directory --

    #[test]
    fn discover_when_manifest_only_nested_then_reports_manifest_not_in_directory() {
        // Matches a real layout found in a private test corpus:
        // TestProject/TestProject/TestProjectRuntime/TestProjectRuntime.plcproj.
        // Opening the tree above the project used to work by accident,
        // via a recursive search that had to guess between candidates.
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Solution").join("Runtime");
        write_plcproj(&nested.join("project.plcproj"), &["MAIN.TcPOU"]);

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6014");
        // The message names the manifest, so the fix is to open its folder.
        assert!(format!("{error:?}").contains("project.plcproj"));
    }

    #[test]
    fn discover_when_manifest_nested_and_loose_sources_present_then_unstructured() {
        // The directory has content of its own, so the nested solution is
        // incidental -- enumerating the loose files is what was asked for
        // and the hint would be a spurious complaint.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();
        let nested = dir.path().join("Unrelated").join("Runtime");
        write_plcproj(&nested.join("project.plcproj"), &["MAIN.TcPOU"]);

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.iter().any(|f| f.ends_with("main.st")));
    }

    #[test]
    fn discover_when_manifest_directory_opened_then_no_hint() {
        // Opening the folder that holds the manifest is the convention
        // the hint exists to teach; it must resolve, not complain.
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Solution").join("Runtime");
        write_plcproj(&nested.join("project.plcproj"), &["MAIN.TcPOU"]);

        let result = discover(&nested).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
    }

    #[test]
    fn discover_when_hidden_directory_contains_plcproj_then_ignored() {
        // .git/.idea-style directories must not be descended into, both
        // for correctness (a decoy .plcproj must not even be named as a
        // hint) and to avoid wastefully/riskily walking a real .git tree.
        let dir = TempDir::new().unwrap();
        let hidden = dir.path().join(".git");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("decoy.plcproj"), "<Project/>").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::Unstructured);
        assert!(result.files.is_empty());
    }

    // -- Multi-project solutions --

    // -- Manifests named directly, rather than found in a directory --

    #[test]
    fn is_manifest_when_manifest_extension_then_true() {
        assert!(is_manifest(Path::new("Solution.sln")));
        assert!(is_manifest(Path::new("Main.TSPROJ")));
        assert!(is_manifest(Path::new("Runtime.plcproj")));
    }

    #[test]
    fn is_manifest_when_source_file_then_false() {
        assert!(!is_manifest(Path::new("main.st")));
        assert!(!is_manifest(Path::new("MAIN.TcPOU")));
        assert!(!is_manifest(Path::new("Safety.splcproj")));
        assert!(!is_manifest(Path::new("no-extension")));
    }

    #[test]
    fn discover_from_manifest_when_sln_then_resolves_chain() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Solution.sln"),
            SOLUTION_NAMING_MAIN_TSPROJ,
        );
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        write_plcproj(
            &dir.path().join("Runtime").join("Runtime.plcproj"),
            &["MAIN.TcPOU"],
        );

        let result = discover_from_manifest(&dir.path().join("Solution.sln")).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("MAIN.TcPOU"));
    }

    #[test]
    fn discover_from_manifest_when_tsproj_then_resolves_its_plcproj() {
        let dir = TempDir::new().unwrap();
        write_file(
            &dir.path().join("Main.tsproj"),
            TSPROJ_NAMING_RUNTIME_PLCPROJ,
        );
        write_plcproj(
            &dir.path().join("Runtime").join("Runtime.plcproj"),
            &["MAIN.TcPOU"],
        );

        let result = discover_from_manifest(&dir.path().join("Main.tsproj")).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("MAIN.TcPOU"));
    }

    #[test]
    fn discover_from_manifest_when_tsproj_names_no_plcproj_then_reports_unresolvable() {
        let dir = TempDir::new().unwrap();
        write_file(&dir.path().join("Main.tsproj"), TSPROJ_NAMING_NOTHING);

        let error = discover_from_manifest(&dir.path().join("Main.tsproj")).unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn discover_from_manifest_when_not_a_manifest_then_reports_unresolvable() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.st"), "PROGRAM END_PROGRAM").unwrap();

        let error = discover_from_manifest(&dir.path().join("main.st")).unwrap_err();

        assert_eq!(error.code, "P6013");
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
