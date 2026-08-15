//! `.sln` -> `.tsproj` -> `.plcproj` resolution for TwinCAT solutions.
//!
//! A `.sln` is the file a user/tool actually points a TwinCAT solution
//! at; it lists one or more Visual Studio-style sub-projects (`.tsproj`
//! among them), and each `.tsproj` in turn names its `.plcproj`
//! sub-projects via nested `PrjFilePath` attributes.
//!
//! Resolution follows that chain and only that chain. The directory tree
//! is never searched for `.plcproj` files: the nesting a real TwinCAT
//! layout has is traversed *by reference*, each manifest naming the next.
//! Searching instead would have to guess when a directory holds more than
//! one candidate, and guessing is exactly what picks a stale `.plcproj`
//! left behind by a project rename over the live one -- see
//! `discover_when_sln_and_stale_duplicate_plcproj_then_picks_named_one`
//! below.

use std::{fs, path::Path, path::PathBuf};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// Find every file directly in `dir` (not recursive) whose extension
/// matches `extension`, sorted by path.
///
/// Deliberately non-recursive: a project manifest is the file a user or
/// tool points at, so discovery looks for it where it was pointed and
/// nowhere else.
pub(super) fn find_manifests(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut manifests: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect();
    manifests.sort();
    manifests
}

/// Resolve the `.plcproj` paths a `.sln` names, via the `.tsproj` files
/// it lists.
///
/// Returns an error if the `.sln` cannot be read, if any `.tsproj` it
/// lists cannot be resolved, or if the chain names no `.plcproj` at all.
/// A `.sln` is authoritative, so a failure here is reported rather than
/// worked around.
pub(super) fn resolve_plcproj_via_sln(sln_path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let tsproj_paths = parse_sln(sln_path)?;

    let mut plcproj_paths = Vec::new();
    for tsproj_path in &tsproj_paths {
        plcproj_paths.extend(resolve_plcproj_via_tsproj(tsproj_path)?);
    }

    if plcproj_paths.is_empty() {
        return Err(unresolvable(
            sln_path,
            "the solution does not name any TwinCAT PLC project (.plcproj)",
        ));
    }

    Ok(plcproj_paths)
}

/// Parse a `.sln` (a line-oriented format, not XML) and return the
/// `.tsproj` paths it lists, resolved relative to the `.sln`'s own
/// directory.
///
/// Each project entry has the form:
/// `Project("{TypeGUID}") = "Name", "RelativePath", "{ProjectGUID}"`.
/// Entries whose `RelativePath` doesn't end in `.tsproj` are other
/// Visual Studio project types the `.sln` may also list (a
/// `DriveManager.tcdmproj`, a `Scope.tcmproj`, ...) and are ignored, not
/// treated as an error.
fn parse_sln(sln_path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let content = fs::read_to_string(sln_path)
        .map_err(|e| unresolvable(sln_path, &format!("cannot read the solution file: {e}")))?;
    let sln_dir = sln_path.parent().unwrap_or(sln_path);

    let mut tsproj_paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("Project(") {
            continue;
        }

        // Quoted segments land at odd indices after splitting on '"':
        // [1]=TypeGUID, [3]=Name, [5]=RelativePath, [7]=ProjectGUID.
        let quoted: Vec<&str> = line.split('"').collect();
        let Some(relative_path) = quoted.get(5) else {
            continue;
        };

        if !has_extension(relative_path, "tsproj") {
            continue;
        }

        let normalized = relative_path.replace('\\', "/");
        tsproj_paths.push(sln_dir.join(normalized));
    }

    Ok(tsproj_paths)
}

/// Parse a `.tsproj` (XML) and return the `.plcproj` files it names via
/// its nested `<Project PrjFilePath="...">` entries, resolved relative
/// to the `.tsproj`'s own directory.
///
/// A `.tsproj` names a sub-project through `PrjFilePath` regardless of
/// nesting depth, so entries are found by attribute presence
/// (`.descendants()`, the same traversal `parse_plcproj` uses for
/// `<Compile>`), not by assuming a fixed nesting shape. Entries whose
/// `PrjFilePath` doesn't end in `.plcproj` -- notably `.splcproj`
/// TwinSAFE safety projects, which use a different compilation model --
/// are skipped, not treated as an error.
///
/// Returns an error if the file cannot be read or parsed as XML. A
/// `.tsproj` that names no `.plcproj` is *not* an error here: a solution
/// may legitimately contain a TwinCAT project with no PLC part, and
/// [`resolve_plcproj_via_sln`] reports the empty overall result instead.
pub(super) fn resolve_plcproj_via_tsproj(tsproj_path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let content = fs::read_to_string(tsproj_path)
        .map_err(|e| unresolvable(tsproj_path, &format!("cannot read the project file: {e}")))?;
    let doc = roxmltree::Document::parse(&content)
        .map_err(|e| unresolvable(tsproj_path, &format!("malformed .tsproj XML: {e}")))?;
    let tsproj_dir = tsproj_path.parent().unwrap_or(tsproj_path);

    let mut plcproj_paths = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let Some(prj_file_path) = node.attribute("PrjFilePath") else {
            continue;
        };

        if !has_extension(prj_file_path, "plcproj") {
            continue;
        }

        let normalized = prj_file_path.replace('\\', "/");
        plcproj_paths.push(tsproj_dir.join(normalized));
    }

    Ok(plcproj_paths)
}

/// Whether a manifest-declared, possibly Windows-style relative path ends
/// in `extension`. Compares the text rather than going through
/// [`Path::extension`], because the value has not been normalized yet.
fn has_extension(declared_path: &str, extension: &str) -> bool {
    declared_path
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

/// A manifest was found and is authoritative, but does not resolve to a
/// project. Never a reason to fall back to a heuristic.
pub(super) fn unresolvable(manifest_path: &Path, reason: &str) -> Diagnostic {
    Diagnostic::problem(
        Problem::ProjectManifestUnresolvable,
        Label::file(
            FileId::from_path(manifest_path),
            format!("{} does not resolve to a project: {reason}", {
                manifest_path.display()
            }),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::fixtures::{write_plcproj, write_sln, write_tsproj};
    use crate::discovery::{discover, ProjectType};
    use tempfile::TempDir;

    #[test]
    fn discover_when_sln_present_then_resolves_via_tsproj() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main\\Main.tsproj")]);

        let tsproj_dir = dir.path().join("Main");
        write_tsproj(
            &tsproj_dir.join("Main.tsproj"),
            &[("MainRuntime", "MainRuntime\\MainRuntime.plcproj")],
        );
        write_plcproj(
            &tsproj_dir.join("MainRuntime").join("MainRuntime.plcproj"),
            &["A.TcPOU"],
        );

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.project_type, ProjectType::TwinCat);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("A.TcPOU"));
    }

    #[test]
    fn discover_when_sln_lists_non_tsproj_project_then_ignored() {
        let dir = TempDir::new().unwrap();
        write_sln(
            dir.path(),
            "Solution.sln",
            &[
                ("Scope", "Scope\\Scope.tcmproj"),
                ("Main", "Main\\Main.tsproj"),
            ],
        );

        let tsproj_dir = dir.path().join("Main");
        write_tsproj(
            &tsproj_dir.join("Main.tsproj"),
            &[("MainRuntime", "MainRuntime\\MainRuntime.plcproj")],
        );
        write_plcproj(
            &tsproj_dir.join("MainRuntime").join("MainRuntime.plcproj"),
            &["A.TcPOU"],
        );
        // Scope.tcmproj deliberately doesn't exist -- a non-.tsproj entry
        // must not be resolved/read at all, only filtered out by extension.

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("A.TcPOU"));
    }

    #[test]
    fn discover_when_sln_and_stale_duplicate_plcproj_then_picks_named_one() {
        // Regression case modeled on a real private-corpus TwinCAT
        // solution: a project renamed Foo -> Fooo left a stale
        // `Foo.plcproj` and `Foo.tsproj` behind, neither referenced by
        // the .sln. The stale file is named so it would sort first,
        // so this fails under plain alphabetical-glob resolution.
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Fooo", "Fooo.tsproj")]);

        // Live .tsproj, referenced by the .sln.
        write_tsproj(
            &dir.path().join("Fooo.tsproj"),
            &[("FoooRuntime", "Runtime\\Fooo.plcproj")],
        );
        // Stale .tsproj, NOT referenced by the .sln -- must be ignored.
        write_tsproj(
            &dir.path().join("Foo.tsproj"),
            &[("FooRuntime", "Runtime\\Foo.plcproj")],
        );

        let plcproj_dir = dir.path().join("Runtime");
        write_plcproj(&plcproj_dir.join("Fooo.plcproj"), &["LIVE.TcPOU"]);
        // Stale duplicate: sorts before "Fooo.plcproj" alphabetically,
        // so a glob-and-sort resolution would have picked this one.
        fs::write(plcproj_dir.join("Foo.plcproj"), "<Project/>").unwrap();

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("LIVE.TcPOU"));
    }

    #[test]
    fn discover_when_tsproj_references_splcproj_then_skipped() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main.tsproj")]);
        write_tsproj(
            &dir.path().join("Main.tsproj"),
            &[
                ("MainRuntime", "MainRuntime\\MainRuntime.plcproj"),
                ("MainTwinSAFE", "MainTwinSAFE\\MainTwinSAFE.splcproj"),
            ],
        );
        write_plcproj(
            &dir.path().join("MainRuntime").join("MainRuntime.plcproj"),
            &["A.TcPOU"],
        );
        // MainTwinSAFE.splcproj deliberately doesn't exist -- proves it
        // was never resolved/read, only filtered out by extension.

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("A.TcPOU"));
    }

    #[test]
    fn discover_when_sln_references_tsproj_with_multiple_plcproj_then_merges_all() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main.tsproj")]);
        write_tsproj(
            &dir.path().join("Main.tsproj"),
            &[
                ("MainRuntime", "MainRuntime\\MainRuntime.plcproj"),
                ("SharedLib", "SharedLib\\SharedLib.plcproj"),
            ],
        );
        write_plcproj(
            &dir.path().join("MainRuntime").join("MainRuntime.plcproj"),
            &["MAIN.TcPOU"],
        );
        write_plcproj(
            &dir.path().join("SharedLib").join("SharedLib.plcproj"),
            &["FB_Shared.TcPOU"],
        );

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ends_with("MAIN.TcPOU")));
        assert!(result.files.iter().any(|f| f.ends_with("FB_Shared.TcPOU")));
    }

    #[test]
    fn discover_when_sln_lists_missing_tsproj_then_reports_unresolvable() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main.tsproj")]);
        // Main.tsproj deliberately absent.

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn discover_when_tsproj_is_malformed_xml_then_reports_unresolvable() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main.tsproj")]);
        fs::write(dir.path().join("Main.tsproj"), "<Project>").unwrap();

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn discover_when_sln_names_no_plcproj_then_reports_unresolvable() {
        let dir = TempDir::new().unwrap();
        // A solution with only a non-PLC project: authoritative, but
        // there is nothing for the compiler to build.
        write_sln(dir.path(), "Solution.sln", &[("Scope", "Scope.tcmproj")]);

        let error = discover(dir.path()).unwrap_err();

        assert_eq!(error.code, "P6013");
    }
}
