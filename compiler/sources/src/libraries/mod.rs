//! Compatibility-library loading.
//!
//! A compatibility library is on-disk data — a `library.toml` manifest plus
//! per-version subdirectories of IEC 61131-3 declarations — that IronPLC ships
//! dormant and injects into a compilation unit when activated, so its symbols
//! resolve under their exact vendor names. Libraries are read from disk at
//! runtime, not embedded in the compiler binary.
//!
//! See `specs/design/compatibility-libraries.md` (`REQ-CL-*`) and
//! `specs/design/compatibility-library-format.md` (`REQ-LF-*`).

pub mod manifest;

use std::fs;
use std::path::{Path, PathBuf};

use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;

use crate::libraries::manifest::LibraryManifest;

/// A loaded compatibility library: its manifest plus the parsed declarations
/// for the selected version, ready to inject into semantic analysis.
#[derive(Debug, Clone)]
pub struct CompatLibrary {
    /// The validated `library.toml` manifest.
    pub manifest: LibraryManifest,
    /// The merged declarations parsed from the selected version's `.st` files.
    pub library: Library,
}

/// A registry of compatibility libraries rooted at a directory.
///
/// Each immediate subdirectory whose name matches a requested library name is a
/// library package (a `library.toml` plus version subdirectories).
#[derive(Debug, Clone)]
pub struct LibraryRegistry {
    root: PathBuf,
}

impl Default for LibraryRegistry {
    fn default() -> Self {
        Self::bundled()
    }
}

impl LibraryRegistry {
    /// The registry of libraries bundled with the compiler.
    ///
    /// Libraries are read from disk at runtime (not embedded in the binary):
    /// the root is the crate's `resources/compat-libraries` directory. The
    /// install location and search mechanism for a shipped binary are defined
    /// separately (out of scope for the on-disk format; see the format design's
    /// *Installation* section).
    pub fn bundled() -> Self {
        LibraryRegistry {
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/compat-libraries"),
        }
    }

    /// A registry rooted at an arbitrary directory (used by tests).
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        LibraryRegistry { root: root.into() }
    }

    /// Whether a library with this exact, case-sensitive name is bundled.
    pub fn contains(&self, name: &str) -> bool {
        self.manifest_path(name).is_file()
    }

    fn manifest_path(&self, name: &str) -> PathBuf {
        self.root.join(name).join("library.toml")
    }

    /// Load a bundled library by its exact, case-sensitive name.
    ///
    /// Resolution to a bundled library is by strict name match
    /// (`REQ-CL-sources-003`), so the compiler never silently binds the wrong
    /// library. Returns a `LibraryNotFound` (P6011) diagnostic when no library
    /// of that name is bundled, or a `LibraryManifestInvalid` (P6010)
    /// diagnostic when the manifest is malformed or missing a required field
    /// (`REQ-CL-sources-002`).
    pub fn load(&self, name: &str) -> Result<CompatLibrary, Diagnostic> {
        let manifest_path = self.manifest_path(name);
        let manifest_file_id = FileId::from_path(&manifest_path);

        if !manifest_path.is_file() {
            return Err(Diagnostic::problem(
                Problem::LibraryNotFound,
                Label::file(
                    manifest_file_id,
                    format!("no bundled compatibility library named `{name}`"),
                ),
            ));
        }

        let content = fs::read_to_string(&manifest_path).map_err(|e| {
            Diagnostic::problem(
                Problem::LibraryManifestInvalid,
                Label::file(
                    manifest_file_id.clone(),
                    format!("cannot read manifest: {e}"),
                ),
            )
        })?;
        let manifest = LibraryManifest::from_toml(&content, &manifest_file_id)?;

        let version_dir = self.root.join(name).join(&manifest.default_version);
        let library = load_version_library(&version_dir, &manifest_file_id)?;

        Ok(CompatLibrary { manifest, library })
    }
}

/// Parse and merge every `.st` file in a version subdirectory into one Library.
///
/// Files are parsed with permissive options so a bundled library's declarations
/// always produce an AST; policy rules (e.g. the top-level `VAR_GLOBAL` gate)
/// are re-checked in the merged analysis under the *user's* options.
fn load_version_library(
    version_dir: &Path,
    manifest_file_id: &FileId,
) -> Result<Library, Diagnostic> {
    let read_dir = fs::read_dir(version_dir).map_err(|e| {
        Diagnostic::problem(
            Problem::LibraryManifestInvalid,
            Label::file(
                manifest_file_id.clone(),
                format!(
                    "cannot read version directory {}: {e}",
                    version_dir.display()
                ),
            ),
        )
    })?;

    let mut paths: Vec<PathBuf> = read_dir.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();

    let options = CompilerOptions::default();
    let mut library = Library::new();
    for path in paths {
        let is_st = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("st"));
        if !is_st {
            continue;
        }
        let file_id = FileId::from_path(&path);
        let content = fs::read_to_string(&path).map_err(|e| {
            Diagnostic::problem(
                Problem::LibraryManifestInvalid,
                Label::file(file_id.clone(), format!("cannot read library file: {e}")),
            )
        })?;
        let parsed = parse_program(&content, &file_id, &options)?;
        library = library.extend(parsed);
    }
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::LibraryElementKind;

    /// The bundled `Tc2_Math` library loads and provides the global `PI`.
    fn library_declares_pi(library: &Library) -> bool {
        library.elements.iter().any(|element| {
            matches!(element, LibraryElementKind::GlobalVarDeclarations(globals)
                if globals.iter().any(|v| v.identifier.symbolic_id().is_some_and(|id| id.original() == "PI")))
        })
    }

    #[test]
    fn bundled_registry_contains_tc2_math() {
        let registry = LibraryRegistry::bundled();
        assert!(registry.contains("Tc2_Math"));
        assert!(!registry.contains("DoesNotExist"));
    }

    #[test]
    fn load_when_tc2_math_then_provides_pi() {
        let registry = LibraryRegistry::bundled();
        let loaded = registry.load("Tc2_Math").expect("Tc2_Math loads");
        assert_eq!(loaded.manifest.name, "Tc2_Math");
        assert_eq!(loaded.manifest.default_version, "1.0.0");
        assert!(
            library_declares_pi(&loaded.library),
            "Tc2_Math must declare the global constant PI"
        );
    }

    #[test]
    fn load_when_case_differs_then_not_found() {
        // Strict, case-sensitive name match (REQ-CL-sources-003).
        let registry = LibraryRegistry::bundled();
        let err = registry.load("tc2_math").unwrap_err();
        assert_eq!(err.code, Problem::LibraryNotFound.code());
    }

    #[test]
    fn load_when_unknown_name_then_library_not_found() {
        let registry = LibraryRegistry::bundled();
        let err = registry.load("Nonexistent").unwrap_err();
        assert_eq!(err.code, Problem::LibraryNotFound.code());
    }

    #[test]
    fn load_when_manifest_invalid_then_manifest_error() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let lib_dir = dir.path().join("Bad");
        fs::create_dir_all(lib_dir.join("1.0.0")).unwrap();
        // Manifest missing the required `default_version` and `references`.
        fs::write(
            lib_dir.join("library.toml"),
            "name = \"Bad\"\nvendor = \"ACME\"\n",
        )
        .unwrap();

        let registry = LibraryRegistry::with_root(dir.path());
        let err = registry.load("Bad").unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn load_when_version_directory_missing_then_error() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let lib_dir = dir.path().join("NoVersion");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(
            lib_dir.join("library.toml"),
            "name = \"NoVersion\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n",
        )
        .unwrap();

        let registry = LibraryRegistry::with_root(dir.path());
        let err = registry.load("NoVersion").unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn load_when_multiple_st_files_then_merges_all() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let version_dir = dir.path().join("Multi").join("1.0.0");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            dir.path().join("Multi").join("library.toml"),
            "name = \"Multi\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n",
        )
        .unwrap();
        fs::write(
            version_dir.join("a.st"),
            "FUNCTION_BLOCK FB_A\nEND_FUNCTION_BLOCK",
        )
        .unwrap();
        fs::write(
            version_dir.join("b.st"),
            "FUNCTION_BLOCK FB_B\nEND_FUNCTION_BLOCK",
        )
        .unwrap();
        // A non-.st file must be ignored.
        fs::write(version_dir.join("notes.txt"), "ignore me").unwrap();

        let registry = LibraryRegistry::with_root(dir.path());
        let loaded = registry.load("Multi").unwrap();
        assert_eq!(loaded.library.elements.len(), 2);
    }
}
