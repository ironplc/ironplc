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

    Ok(tsproj_paths_in_sln(
        &content,
        sln_path.parent().unwrap_or(sln_path),
    ))
}

/// The `.tsproj` paths a `.sln`'s text names, resolved against
/// `sln_dir`. Split from [`parse_sln`] so the format can be exercised
/// against literal `.sln` text, with no file to write or read.
fn tsproj_paths_in_sln(content: &str, sln_dir: &Path) -> Vec<PathBuf> {
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

    tsproj_paths
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

    plcproj_paths_in_tsproj(&content, tsproj_path)
}

/// The `.plcproj` paths a `.tsproj`'s text names, resolved against the
/// `.tsproj`'s own directory. Split from [`resolve_plcproj_via_tsproj`]
/// so the format can be exercised against literal `.tsproj` text, with
/// no file to write or read. `tsproj_path` is still needed to anchor a
/// malformed-XML diagnostic and to resolve relative paths.
fn plcproj_paths_in_tsproj(content: &str, tsproj_path: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let doc = roxmltree::Document::parse(content)
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

    /// A TcXaeShell solution as the IDE writes one, listing a TwinCAT
    /// project alongside a Measurement project. Kept verbatim rather than
    /// generated: the `Global` sections and the non-`.tsproj` entry are
    /// exactly the noise resolution has to step over, and a generator
    /// that emits only the lines we already parse could not go stale in
    /// the way a real file can.
    const SOLUTION_SLN: &str = r#"
Microsoft Visual Studio Solution File, Format Version 12.00
# TcXaeShell Solution File, Format Version 11.00
VisualStudioVersion = 15.0.27130.2036
MinimumVisualStudioVersion = 10.0.40219.1
Project("{9F4CFF6B-4A82-4B4E-9E5A-9C0E2B0D8A97}") = "Scope", "Scope\Scope.tcmproj", "{2A4F0C7E-4F6B-4E5D-9B6C-1E4D8A2F0B31}"
EndProject
Project("{B1E792BE-AA5F-4E3C-8C82-674BF9C0715B}") = "Main", "Main\Main.tsproj", "{9406D69C-EBA9-4591-A513-578A75D14426}"
EndProject
Global
	GlobalSection(SolutionConfigurationPlatforms) = preSolution
		Debug|TwinCAT RT (x64) = Debug|TwinCAT RT (x64)
	EndGlobalSection
	GlobalSection(ProjectConfigurationPlatforms) = postSolution
		{9406D69C-EBA9-4591-A513-578A75D14426}.Debug|TwinCAT RT (x64).ActiveCfg = Debug|TwinCAT RT (x64)
	EndGlobalSection
EndGlobal
"#;

    /// A `.tsproj` naming a PLC project and a TwinSAFE safety project,
    /// nested the way TwinCAT nests them under `<Plc>`.
    const MAIN_TSPROJ: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<TcSmProject ProjectGUID="{9406D69C-EBA9-4591-A513-578A75D14426}">
  <Project>
    <Plc>
      <Project GUID="{6DADE760-7FAC-4830-92BA-478C8595D673}" Name="MainRuntime" PrjFilePath="MainRuntime\MainRuntime.plcproj" TmcFilePath="MainRuntime\MainRuntime.tmc" ReloadTmc="true" AmsPort="851" FileArchiveSettings="#x000e" />
      <Project GUID="{1F2E3D4C-5B6A-4978-8899-AABBCCDDEEFF}" Name="SharedLib" PrjFilePath="SharedLib\SharedLib.plcproj" AmsPort="852" />
      <Project GUID="{0A1B2C3D-4E5F-4A6B-8C9D-0E1F2A3B4C5D}" Name="MainSafety" PrjFilePath="MainSafety\MainSafety.splcproj" AmsPort="853" />
    </Plc>
  </Project>
</TcSmProject>
"##;

    #[test]
    fn tsproj_paths_in_sln_when_solution_then_resolves_relative_to_sln_directory() {
        let paths = tsproj_paths_in_sln(SOLUTION_SLN, Path::new("/solutions/MySolution"));

        // The one `.tsproj` entry, with its Windows-style separator
        // normalized and resolved against the .sln's own directory.
        assert_eq!(
            paths,
            vec![PathBuf::from("/solutions/MySolution/Main/Main.tsproj")]
        );
    }

    #[test]
    fn tsproj_paths_in_sln_when_non_tsproj_entry_then_ignored() {
        // The Scope.tcmproj entry is another Visual Studio project type
        // the solution may list; it is filtered out by extension and
        // never resolved, so its absence on disk is not an error.
        let paths = tsproj_paths_in_sln(SOLUTION_SLN, Path::new("/solutions/MySolution"));

        assert!(!paths.iter().any(|path| path.ends_with("Scope.tcmproj")));
    }

    #[test]
    fn tsproj_paths_in_sln_when_no_project_entries_then_empty() {
        let paths = tsproj_paths_in_sln(
            "Microsoft Visual Studio Solution File, Format Version 12.00\nGlobal\nEndGlobal\n",
            Path::new("/solutions/MySolution"),
        );

        assert!(paths.is_empty());
    }

    #[test]
    fn plcproj_paths_in_tsproj_when_project_entries_then_resolves_relative_to_tsproj_directory() {
        let paths =
            plcproj_paths_in_tsproj(MAIN_TSPROJ, Path::new("/solutions/MySolution/Main.tsproj"))
                .unwrap();

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/solutions/MySolution/MainRuntime/MainRuntime.plcproj"),
                PathBuf::from("/solutions/MySolution/SharedLib/SharedLib.plcproj"),
            ]
        );
    }

    #[test]
    fn plcproj_paths_in_tsproj_when_splcproj_entry_then_skipped() {
        // TwinSAFE safety projects use a different compilation model and
        // are out of scope -- filtered out by extension, not an error.
        let paths =
            plcproj_paths_in_tsproj(MAIN_TSPROJ, Path::new("/solutions/MySolution/Main.tsproj"))
                .unwrap();

        assert!(!paths
            .iter()
            .any(|path| path.ends_with("MainSafety.splcproj")));
    }

    #[test]
    fn plcproj_paths_in_tsproj_when_no_project_entries_then_empty() {
        // A TwinCAT project with no PLC part. Not an error here --
        // `resolve_plcproj_via_sln` reports the empty overall result.
        let paths = plcproj_paths_in_tsproj(
            r#"<TcSmProject><Project><Io /></Project></TcSmProject>"#,
            Path::new("/solutions/MySolution/Main.tsproj"),
        )
        .unwrap();

        assert!(paths.is_empty());
    }

    #[test]
    fn plcproj_paths_in_tsproj_when_malformed_xml_then_reports_unresolvable() {
        let error = plcproj_paths_in_tsproj(
            "<TcSmProject>",
            Path::new("/solutions/MySolution/Main.tsproj"),
        )
        .unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn resolve_plcproj_via_tsproj_when_file_missing_then_reports_unresolvable() {
        // A .sln naming a .tsproj that isn't there. Nothing is written:
        // the path simply does not exist.
        let error = resolve_plcproj_via_tsproj(Path::new("/nonexistent/MySolution/Main.tsproj"))
            .unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn resolve_plcproj_via_sln_when_file_missing_then_reports_unresolvable() {
        let error = resolve_plcproj_via_sln(Path::new("/nonexistent/MySolution.sln")).unwrap_err();

        assert_eq!(error.code, "P6013");
    }

    #[test]
    fn tsproj_paths_in_sln_when_stale_rename_left_behind_then_only_named_one_resolved() {
        // Regression case modeled on a real private-corpus TwinCAT
        // solution: a project renamed Foo -> Fooo left a stale
        // `Foo.tsproj` and `Foo.plcproj` behind, neither referenced by
        // the .sln. The stale name sorts first, so a glob-and-sort
        // resolution picks it; resolution by reference cannot, because
        // the stale file is never named by anything.
        //
        // `discover_when_sln_and_plcproj_in_same_directory_then_sln_wins`
        // in `mod.rs` covers the same guarantee end-to-end, with the
        // stale file physically present on disk.
        let sln = r#"Microsoft Visual Studio Solution File, Format Version 12.00
Project("{B1E792BE-AA5F-4E3C-8C82-674BF9C0715B}") = "Fooo", "Fooo.tsproj", "{9406D69C-EBA9-4591-A513-578A75D14426}"
EndProject
"#;

        let paths = tsproj_paths_in_sln(sln, Path::new("/solutions/Foo"));

        assert_eq!(paths, vec![PathBuf::from("/solutions/Foo/Fooo.tsproj")]);
    }
}
