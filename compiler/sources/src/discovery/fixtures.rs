//! Shared fixture builders for the discovery tests.
//!
//! Discovery is defined by what a manifest chain says, so almost every
//! test needs a `.sln`, a `.tsproj`, or a `.plcproj` on disk with real
//! content. These write the minimum shape of each that discovery reads.

use std::fs;
use std::path::Path;

/// Write a `.sln` listing one `Project(...)` entry per
/// `(name, relative_path)`. Paths use the Windows-style separators a
/// real solution file carries.
pub(super) fn write_sln(dir: &Path, name: &str, project_entries: &[(&str, &str)]) {
    let mut content = String::from(
        "Microsoft Visual Studio Solution File, Format Version 12.00\n\
         # TcXaeShell Solution File, Format Version 11.00\n",
    );
    for (project_name, relative_path) in project_entries {
        content.push_str(&format!(
            "Project(\"{{B1E792BE-AA5F-4E3C-8C82-674BF9C0715B}}\") = \"{project_name}\", \"{relative_path}\", \"{{9406D69C-EBA9-4591-A513-578A75D14426}}\"\nEndProject\n"
        ));
    }
    fs::write(dir.join(name), content).unwrap();
}

/// Write a `.tsproj` at `path` naming one sub-project per
/// `(name, prj_file_path)`.
pub(super) fn write_tsproj(path: &Path, plc_entries: &[(&str, &str)]) {
    let mut inner = String::new();
    for (name, prj_file_path) in plc_entries {
        inner.push_str(&format!(
            r#"<Project GUID="{{6DADE760-7FAC-4830-92BA-478C8595D673}}" Name="{name}" PrjFilePath="{prj_file_path}" />"#
        ));
    }
    let content = format!(
        r#"<Project ProjectGUID="{{9406D69C-EBA9-4591-A513-578A75D14426}}">{inner}</Project>"#
    );
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Write a `.plcproj` at `path` declaring one `<Compile>` entry per name
/// in `sources`, creating each named file alongside it.
pub(super) fn write_plcproj(path: &Path, sources: &[&str]) {
    write_plcproj_with_item_group(path, &compile_entries(sources));
    let dir = path.parent().unwrap();
    for source in sources {
        let source_path = dir.join(source.replace('\\', "/"));
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(source_path, "<TcPlcObject/>").unwrap();
    }
}

/// Write a `.plcproj` at `path` whose `<ItemGroup>` holds `item_group`
/// verbatim, creating no source files. For tests that need entries
/// discovery cannot resolve, or elements other than `<Compile>`.
pub(super) fn write_plcproj_with_item_group(path: &Path, item_group: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
{item_group}
  </ItemGroup>
</Project>"#
        ),
    )
    .unwrap();
}

/// `<Compile Include="...">` elements for each name, as
/// [`write_plcproj_with_item_group`] expects them.
pub(super) fn compile_entries(sources: &[&str]) -> String {
    sources
        .iter()
        .map(|source| format!(r#"    <Compile Include="{source}" />"#))
        .collect::<Vec<String>>()
        .join("\n")
}
