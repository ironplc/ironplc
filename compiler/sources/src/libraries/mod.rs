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

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;

use crate::libraries::manifest::LibraryManifest;

/// The name of a compatibility library (e.g. `Tc2_System`).
///
/// A distinct type from a filesystem path or an arbitrary string: it is the
/// vendor-facing identifier a project reference or a `--library` option names,
/// matched against a library package by strict, case-sensitive equality
/// (`REQ-CL-sources-003`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LibraryName(String);

impl LibraryName {
    /// Create a library name from any string-like value.
    pub fn new(name: impl Into<String>) -> Self {
        LibraryName(name.into())
    }

    /// The name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for LibraryName {
    fn from(value: &str) -> Self {
        LibraryName(value.to_string())
    }
}

impl From<String> for LibraryName {
    fn from(value: String) -> Self {
        LibraryName(value)
    }
}

impl FromStr for LibraryName {
    // A library name is any non-empty token; parsing is infallible so the CLI
    // can collect `Vec<LibraryName>` directly.
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(LibraryName(s.to_string()))
    }
}

/// A compatibility-library reference declared by a project file.
///
/// This is the *statement of intent* read from a discovered `.plcproj` — the
/// vendor's own record of which libraries the project uses. It is resolved
/// against the bundled registry by [`LibraryRegistry::resolve_references`];
/// the version and namespace are captured for completeness (and future
/// qualified-access work) but do not participate in matching today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryReference {
    /// The referenced library's name, matched against a bundled package by
    /// strict, case-sensitive equality (`REQ-CL-sources-003`).
    pub name: LibraryName,
    /// The version as declared in the project — commonly the `*` wildcard for a
    /// `PlaceholderReference`, or a pinned version for a `LibraryReference`.
    /// `None` when the project states no version. Not used to select a package.
    pub version: Option<String>,
    /// The namespace the source may qualify with (the `<Namespace>` element).
    /// Captured for future qualified-access support; unused in the first
    /// increment, which injects flat names only.
    pub namespace: Option<String>,
    /// The project file the reference was declared in, used to anchor a
    /// diagnostic when the referenced library is not bundled.
    pub declared_in: FileId,
}

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
    /// The registry of libraries installed alongside the application.
    ///
    /// Libraries are read from disk at runtime (not embedded in the binary).
    /// The installed layout places them in `resources/libs` next to the
    /// application executable, so the root is derived from the running
    /// executable's directory. When that directory does not exist — a
    /// development build or the test harness, where the executable is under
    /// `target/` with no resources beside it — the search falls back to the
    /// crate's own `resources/libs` source directory.
    pub fn bundled() -> Self {
        LibraryRegistry {
            root: installed_libraries_root(),
        }
    }

    /// A registry rooted at an arbitrary directory (used by tests).
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        LibraryRegistry { root: root.into() }
    }

    /// Whether a library with this exact, case-sensitive name is available.
    pub fn contains(&self, name: &LibraryName) -> bool {
        self.has_exact_dir(name) && self.manifest_path(name).is_file()
    }

    /// Every bundled library available under the registry root.
    ///
    /// A library is an immediate subdirectory that holds a `library.toml`
    /// manifest; the returned names are sorted so a walk over them is stable.
    /// Used by the provenance conformance test to enforce the authoring policy
    /// across every bundled manifest (`REQ-CL-sources-007`).
    pub fn library_names(&self) -> Vec<LibraryName> {
        let mut names: Vec<LibraryName> = match fs::read_dir(&self.root) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().join("library.toml").is_file())
                .filter_map(|entry| entry.file_name().to_str().map(LibraryName::from))
                .collect(),
            Err(_) => Vec::new(),
        };
        names.sort();
        names
    }

    fn manifest_path(&self, name: &LibraryName) -> PathBuf {
        self.root.join(name.as_str()).join("library.toml")
    }

    /// Whether an immediate subdirectory named exactly (byte-for-byte) equal to
    /// `name` exists under the registry root.
    ///
    /// Name matching must be strict and case-sensitive (`REQ-CL-sources-003`),
    /// but `Path::is_file`/`is_dir` resolve case-insensitively on macOS (APFS)
    /// and Windows, so `tc2_system` would match a `Tc2_System` directory there.
    /// Confirming the real on-disk entry name makes the match case-sensitive on
    /// every platform.
    fn has_exact_dir(&self, name: &LibraryName) -> bool {
        match fs::read_dir(&self.root) {
            Ok(entries) => entries.filter_map(Result::ok).any(|entry| {
                entry.file_name().to_str() == Some(name.as_str()) && entry.path().is_dir()
            }),
            Err(_) => false,
        }
    }

    /// Load a library by its exact, case-sensitive name.
    ///
    /// Resolution is by strict name match (`REQ-CL-sources-003`), so the
    /// compiler never silently binds the wrong library. Returns a
    /// `LibraryNotFound` (P6011) diagnostic when no library of that name is
    /// available, or a `LibraryManifestInvalid` (P6010) diagnostic when the
    /// manifest is malformed or missing a required field (`REQ-CL-sources-002`).
    pub fn load(&self, name: &LibraryName) -> Result<CompatLibrary, Diagnostic> {
        let manifest_path = self.manifest_path(name);
        let manifest_file_id = FileId::from_path(&manifest_path);

        // Strict, case-sensitive name match (`REQ-CL-sources-003`): the exact
        // directory-entry check guards against case-insensitive filesystems
        // that would otherwise bind e.g. `tc2_system` to `Tc2_System`.
        if !self.has_exact_dir(name) || !manifest_path.is_file() {
            return Err(Diagnostic::problem(
                Problem::LibraryNotFound,
                Label::file(
                    manifest_file_id,
                    format!("compatibility library `{name}` not found"),
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

        let version_dir = self
            .root
            .join(name.as_str())
            .join(&manifest.default_version);
        let library = load_version_library(&version_dir, &manifest_file_id)?;

        Ok(CompatLibrary { manifest, library })
    }

    /// Resolve project-declared library references to the set of bundled library
    /// names to activate, diagnosing any reference this build does not bundle.
    ///
    /// Matching is by strict, case-sensitive **name** (`REQ-CL-sources-003`);
    /// the reference's version does not select a package, so a `*` (the common
    /// `PlaceholderReference` wildcard) or any pinned version resolves to the
    /// single bundled version by name alone. Returned names are deduplicated and
    /// in first-seen order.
    ///
    /// A referenced library that is not bundled yields a `LibraryNotFound`
    /// (P6011) diagnostic that names it (`REQ-CL-sources-004`), rather than
    /// failing silently, so any resulting undefined-symbol errors are explained.
    pub fn resolve_references(
        &self,
        references: &[LibraryReference],
    ) -> (Vec<LibraryName>, Vec<Diagnostic>) {
        let mut activated: Vec<LibraryName> = Vec::new();
        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        for reference in references {
            if self.contains(&reference.name) {
                if !activated.contains(&reference.name) {
                    activated.push(reference.name.clone());
                }
            } else {
                diagnostics.push(Diagnostic::problem(
                    Problem::LibraryNotFound,
                    Label::file(
                        reference.declared_in.clone(),
                        format!(
                            "project references compatibility library `{}`, which IronPLC does not bundle",
                            reference.name
                        ),
                    ),
                ));
            }
        }

        (activated, diagnostics)
    }
}

/// The root directory holding installed compatibility libraries.
///
/// Prefers `resources/libs` beside the running executable (the installed
/// layout); falls back to the crate's `resources/libs` source directory for
/// development and test builds, where the executable lives under `target/`
/// with no resources beside it.
fn installed_libraries_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let installed = exe_dir.join("resources").join("libs");
            if installed.is_dir() {
                return installed;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/libs")
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

    #[test]
    fn library_name_round_trips_through_str() {
        let name = LibraryName::from("Tc2_System");
        assert_eq!(name.as_str(), "Tc2_System");
        assert_eq!(name.to_string(), "Tc2_System");
        assert_eq!("Tc2_System".parse::<LibraryName>().unwrap(), name);
        assert_eq!(LibraryName::new(String::from("Tc2_System")), name);
    }

    /// The bundled `Tc2_System` library loads and provides the global `PI`.
    fn library_declares_pi(library: &Library) -> bool {
        library.elements.iter().any(|element| {
            matches!(element, LibraryElementKind::GlobalVarDeclarations(globals)
                if globals.iter().any(|v| v.identifier.symbolic_id().is_some_and(|id| id.original() == "PI")))
        })
    }

    /// Whether the library declares a FUNCTION with the given name.
    fn library_declares_function(library: &Library, name: &str) -> bool {
        library.elements.iter().any(|element| {
            matches!(element, LibraryElementKind::FunctionDeclaration(function)
                if function.name.original() == name)
        })
    }

    #[test]
    fn bundled_registry_contains_tc2_system() {
        let registry = LibraryRegistry::bundled();
        assert!(registry.contains(&LibraryName::from("Tc2_System")));
        assert!(!registry.contains(&LibraryName::from("DoesNotExist")));
    }

    #[test]
    fn bundled_registry_contains_tc2_builtins() {
        let registry = LibraryRegistry::bundled();
        assert!(registry.contains(&LibraryName::from("Tc2_BuiltIns")));
    }

    #[test]
    fn load_when_tc2_builtins_then_provides_bool_to_string() {
        let registry = LibraryRegistry::bundled();
        let loaded = registry.load(&LibraryName::from("Tc2_BuiltIns")).unwrap();
        assert_eq!(loaded.manifest.name, "Tc2_BuiltIns");
        assert_eq!(loaded.manifest.default_version, "1.0.0");
        assert!(
            library_declares_function(&loaded.library, "BOOL_TO_STRING"),
            "Tc2_BuiltIns must declare the function BOOL_TO_STRING"
        );
    }

    #[test]
    fn load_when_tc2_system_then_provides_pi() {
        let registry = LibraryRegistry::bundled();
        let loaded = registry.load(&LibraryName::from("Tc2_System")).unwrap();
        assert_eq!(loaded.manifest.name, "Tc2_System");
        assert_eq!(loaded.manifest.default_version, "1.0.0");
        assert!(
            library_declares_pi(&loaded.library),
            "Tc2_System must declare the global constant PI"
        );
    }

    #[test]
    fn load_when_case_differs_then_not_found() {
        // Strict, case-sensitive name match (REQ-CL-sources-003).
        let registry = LibraryRegistry::bundled();
        let err = registry.load(&LibraryName::from("tc2_system")).unwrap_err();
        assert_eq!(err.code, Problem::LibraryNotFound.code());
    }

    #[test]
    fn load_when_unknown_name_then_library_not_found() {
        let registry = LibraryRegistry::bundled();
        let err = registry
            .load(&LibraryName::from("Nonexistent"))
            .unwrap_err();
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
        let err = registry.load(&LibraryName::from("Bad")).unwrap_err();
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
        let err = registry.load(&LibraryName::from("NoVersion")).unwrap_err();
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
        let loaded = registry.load(&LibraryName::from("Multi")).unwrap();
        assert_eq!(loaded.library.elements.len(), 2);
    }

    fn reference(name: &str) -> LibraryReference {
        LibraryReference {
            name: LibraryName::from(name),
            version: Some("*".to_string()),
            namespace: None,
            declared_in: FileId::from_string("proj.plcproj"),
        }
    }

    #[test]
    fn resolve_references_when_bundled_then_activates() {
        let registry = LibraryRegistry::bundled();
        let (activated, diagnostics) = registry.resolve_references(&[reference("Tc2_System")]);
        assert_eq!(activated, [LibraryName::from("Tc2_System")]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn resolve_references_when_unshipped_then_diagnoses_by_name() {
        let registry = LibraryRegistry::bundled();
        let (activated, diagnostics) = registry.resolve_references(&[reference("Nonexistent")]);
        assert!(activated.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::LibraryNotFound.code());
        assert!(diagnostics[0].primary.message.contains("Nonexistent"));
    }

    #[test]
    fn resolve_references_when_duplicate_then_deduplicated() {
        let registry = LibraryRegistry::bundled();
        let (activated, diagnostics) =
            registry.resolve_references(&[reference("Tc2_System"), reference("Tc2_System")]);
        assert_eq!(activated, [LibraryName::from("Tc2_System")]);
        assert!(diagnostics.is_empty());
    }
}
