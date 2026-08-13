//! `.sln` -> `.tsproj` -> `.plcproj` resolution for TwinCAT solutions.
//!
//! A `.sln` is the file a user/tool actually points a TwinCAT solution
//! at; it lists one or more Visual Studio-style sub-projects (`.tsproj`
//! among them), and each `.tsproj` in turn names its `.plcproj`
//! sub-projects via nested `PrjFilePath` attributes. Resolving through
//! this chain (rather than just walking the directory tree for
//! `.plcproj` files) is what lets discovery pick the correct, currently
//! referenced `.plcproj` when a directory also contains stale,
//! no-longer-referenced project files left behind by a rename -- see
//! `discover_when_sln_and_stale_duplicate_plcproj_then_picks_named_one`
//! below.

use std::{fs, path::Path, path::PathBuf};

/// Find the single `.sln` file directly in `dir` (not recursive -- a
/// `.sln` is always the file a user/tool points at directly, unlike
/// `.plcproj`, which real layouts commonly nest several levels deep).
///
/// Returns `None` if there is no `.sln`, or more than one: ambiguous,
/// callers should fall back to the recursive walk rather than guess.
fn find_sln(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;

    let mut sln_files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("sln"))
        })
        .collect();

    if sln_files.len() != 1 {
        return None;
    }
    sln_files.pop()
}

/// Resolve `.plcproj` paths via `.sln` -> `.tsproj` -> `PrjFilePath`.
///
/// Returns an empty vec if there's no unambiguous `.sln` in `dir`, or
/// the `.sln` found doesn't lead to any `.plcproj` files -- callers
/// should fall back to [`super::collect_plcproj_via_walk`] in that case.
pub(super) fn resolve_plcproj_via_sln(dir: &Path) -> Vec<PathBuf> {
    let Some(sln_path) = find_sln(dir) else {
        return Vec::new();
    };

    parse_sln(&sln_path)
        .into_iter()
        .flat_map(|tsproj_path| resolve_plcproj_via_tsproj(&tsproj_path))
        .collect()
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
fn parse_sln(sln_path: &Path) -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(sln_path) else {
        return Vec::new();
    };
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

        let is_tsproj = relative_path
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tsproj"));
        if !is_tsproj {
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
/// Returns an empty vec if the file can't be read or parsed as XML.
fn resolve_plcproj_via_tsproj(tsproj_path: &Path) -> Vec<PathBuf> {
    let Ok(content) = fs::read_to_string(tsproj_path) else {
        return Vec::new();
    };
    let Ok(doc) = roxmltree::Document::parse(&content) else {
        return Vec::new();
    };
    let tsproj_dir = tsproj_path.parent().unwrap_or(tsproj_path);

    let mut plcproj_paths = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let Some(prj_file_path) = node.attribute("PrjFilePath") else {
            continue;
        };

        let is_plcproj = prj_file_path
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("plcproj"));
        if !is_plcproj {
            continue;
        }

        let normalized = prj_file_path.replace('\\', "/");
        plcproj_paths.push(tsproj_dir.join(normalized));
    }

    plcproj_paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{discover, ProjectType};
    use tempfile::TempDir;

    fn write_sln(dir: &Path, name: &str, tsproj_entries: &[(&str, &str)]) {
        let mut content = String::from(
            "Microsoft Visual Studio Solution File, Format Version 12.00\n\
             # TcXaeShell Solution File, Format Version 11.00\n",
        );
        for (project_name, relative_path) in tsproj_entries {
            content.push_str(&format!(
                "Project(\"{{B1E792BE-AA5F-4E3C-8C82-674BF9C0715B}}\") = \"{project_name}\", \"{relative_path}\", \"{{9406D69C-EBA9-4591-A513-578A75D14426}}\"\nEndProject\n"
            ));
        }
        fs::write(dir.join(name), content).unwrap();
    }

    fn write_tsproj(path: &Path, plc_entries: &[(&str, &str)]) {
        let mut inner = String::new();
        for (name, prj_file_path) in plc_entries {
            inner.push_str(&format!(
                r#"<Project GUID="{{6DADE760-7FAC-4830-92BA-478C8595D673}}" Name="{name}" PrjFilePath="{prj_file_path}" />"#
            ));
        }
        let content = format!(
            r#"<Project ProjectGUID="{{9406D69C-EBA9-4591-A513-578A75D14426}}">{inner}</Project>"#
        );
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discover_when_sln_present_then_resolves_via_tsproj() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "Solution.sln", &[("Main", "Main\\Main.tsproj")]);

        let tsproj_dir = dir.path().join("Main");
        fs::create_dir_all(&tsproj_dir).unwrap();
        write_tsproj(
            &tsproj_dir.join("Main.tsproj"),
            &[("MainRuntime", "MainRuntime\\MainRuntime.plcproj")],
        );

        let plcproj_dir = tsproj_dir.join("MainRuntime");
        fs::create_dir_all(&plcproj_dir).unwrap();
        fs::write(plcproj_dir.join("A.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            plcproj_dir.join("MainRuntime.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="A.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

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
        fs::create_dir_all(&tsproj_dir).unwrap();
        write_tsproj(
            &tsproj_dir.join("Main.tsproj"),
            &[("MainRuntime", "MainRuntime\\MainRuntime.plcproj")],
        );

        let plcproj_dir = tsproj_dir.join("MainRuntime");
        fs::create_dir_all(&plcproj_dir).unwrap();
        fs::write(plcproj_dir.join("A.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            plcproj_dir.join("MainRuntime.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="A.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
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
        fs::create_dir_all(&plcproj_dir).unwrap();
        fs::write(plcproj_dir.join("LIVE.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            plcproj_dir.join("Fooo.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="LIVE.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
        // Stale duplicate: sorts before "Fooo.plcproj" alphabetically,
        // so the old glob-only logic would have picked this one.
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

        let plcproj_dir = dir.path().join("MainRuntime");
        fs::create_dir_all(&plcproj_dir).unwrap();
        fs::write(plcproj_dir.join("A.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            plcproj_dir.join("MainRuntime.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="A.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
        // MainTwinSAFE.splcproj deliberately doesn't exist -- proves it
        // was never resolved/read, only filtered out by extension.

        let result = discover(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].ends_with("A.TcPOU"));
    }

    #[test]
    fn discover_when_multiple_sln_at_top_level_then_falls_back_to_walk() {
        let dir = TempDir::new().unwrap();
        write_sln(dir.path(), "A.sln", &[]);
        write_sln(dir.path(), "B.sln", &[]);

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

        let main_dir = dir.path().join("MainRuntime");
        fs::create_dir_all(&main_dir).unwrap();
        fs::write(main_dir.join("MAIN.TcPOU"), "<TcPlcObject/>").unwrap();
        fs::write(
            main_dir.join("MainRuntime.plcproj"),
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

        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ends_with("MAIN.TcPOU")));
        assert!(result.files.iter().any(|f| f.ends_with("FB_Shared.TcPOU")));
    }
}
