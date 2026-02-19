//! Project discovery pipeline
//!
//! Detects existing PLC project structures (Beremiz, TwinCAT) in a directory
//! and returns the set of source files to load. When no specific project
//! structure is detected, falls back to enumerating all supported files.
//!
//! The detector chain runs in priority order: Beremiz → TwinCAT → Fallback.
//! The first match wins.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use log::{info, trace};

use crate::file_type::FileType;

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
        })
    } else {
        None
    }
}

/// Detect a TwinCAT project by searching for a `.plcproj` file.
///
/// Returns `None` if no `.plcproj` exists. Returns `Some(Err(...))` if
/// the `.plcproj` is found but malformed or references missing files.
fn detect_twincat(dir: &Path) -> Option<Result<DiscoveredProject, Diagnostic>> {
    let entries = fs::read_dir(dir).ok()?;

    let plcproj = entries.filter_map(Result::ok).find(|entry| {
        trace!("Check if file {entry:?} is plcproj");
        entry
            .path()
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("plcproj"))
    })?;

    let plcproj_path = plcproj.path();
    Some(parse_plcproj(&plcproj_path, dir))
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

    // Find all <Compile Include="..."> elements anywhere in the document
    for node in doc.descendants() {
        if node.is_element() && node.tag_name().name() == "Compile" {
            if let Some(include) = node.attribute("Include") {
                // Resolve relative to the .plcproj directory, normalizing
                // Windows-style backslash separators
                let normalized = include.replace('\\', "/");
                let resolved = root_dir.join(&normalized);

                if !resolved.is_file() {
                    return Err(Diagnostic::problem(
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
                }

                files.push(resolved);
            }
        }
    }

    Ok(DiscoveredProject {
        project_type: ProjectType::TwinCat,
        root_dir: root_dir.to_path_buf(),
        files,
    })
}

/// Fallback detection: enumerate all supported files in the directory.
///
/// Returns files sorted alphabetically for deterministic ordering.
fn detect_fallback(dir: &Path) -> DiscoveredProject {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter(|entry| entry.path().is_file() && FileType::from_path(&entry.path()).is_supported())
        .map(|entry| entry.path())
        .collect();

    files.sort();

    info!("Fallback detection found {} supported files", files.len());

    DiscoveredProject {
        project_type: ProjectType::Unstructured,
        root_dir: dir.to_path_buf(),
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
    fn discover_when_plcproj_references_missing_file_then_returns_diagnostic() {
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

        let result = discover(dir.path());
        assert!(result.is_err());

        let diag = result.unwrap_err();
        assert!(diag.primary.message.contains("MISSING.TcPOU"));
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
}
