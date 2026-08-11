//! Compatibility-library declare-only bindings.
//!
//! A library may declare a POU whose implementation has not been built yet:
//! the manifest marks it `"declare-only"` in the per-version bindings table
//! (see `specs/design/compatibility-library-format.md`). The declaration
//! carries the full signature, so `check` and type resolution work
//! unchanged; *calling* the POU is the dedicated compile error P4046 —
//! never silently-wrong codegen and never a runtime trap.
//!
//! Binding information travels *out of band*: the analyze merge erases which
//! POUs came from a library, and provenance markers in the AST are
//! forbidden, so the loader produces this side-table and the driver threads
//! it into codegen. The analyzer never sees bindings — which is exactly what
//! makes a declare-only call pass `check`.
//!
//! Bindings deliberately cannot select an implementation: an earlier design
//! mapped POUs to native VM builtins through the manifest, which was
//! rejected because it made an on-disk data file an input to code emission.
//! Native behavior is exposed instead as typed `__`-prefixed compiler
//! intrinsics that library ST bodies call.

use std::collections::{HashMap, HashSet};

use crate::core::FileId;

/// The declare-only POUs of every activated library, keyed by uppercased
/// POU name, plus the set of library source files.
///
/// The file set serves two purposes: it lets codegen skip compiling a
/// declare-only POU's empty `;` body (so the name is not registered as a
/// callable user function, and the call site reaches the P4046 check), and
/// it preserves user shadowing — the skip applies only to declarations in
/// library files, so a user-defined POU with the same name still compiles
/// as the user's function.
///
/// `Default` is the empty set, so codegen consumers that never activate a
/// library (benchmarks, direct `compile()` callers) are unaffected.
#[derive(Debug, Clone, Default)]
pub struct LibraryBindings {
    /// Uppercased POU name → declaring library's name (used by the P4046
    /// diagnostic).
    declare_only: HashMap<String, String>,
    /// Every `.st` file of every activated library (bound or not).
    library_files: HashSet<FileId>,
}

impl LibraryBindings {
    /// An empty bindings set.
    pub fn new() -> Self {
        Self::default()
    }

    /// True when no library declared any binding and no library file is
    /// registered.
    pub fn is_empty(&self) -> bool {
        self.declare_only.is_empty() && self.library_files.is_empty()
    }

    /// Registers a declare-only POU (matched case-insensitively via
    /// uppercasing, like the function environment) with its declaring
    /// library's name.
    pub fn insert_declare_only(&mut self, pou_name: &str, library: impl Into<String>) {
        self.declare_only
            .insert(pou_name.to_uppercase(), library.into());
    }

    /// The declaring library's name when the POU is declare-only
    /// (case-insensitive lookup).
    pub fn get_declare_only(&self, pou_name: &str) -> Option<&str> {
        self.declare_only
            .get(&pou_name.to_uppercase())
            .map(String::as_str)
    }

    /// Registers a library source file.
    pub fn add_library_file(&mut self, file_id: FileId) {
        self.library_files.insert(file_id);
    }

    /// True when the file is a registered library source file.
    pub fn is_library_file(&self, file_id: &FileId) -> bool {
        self.library_files.contains(file_id)
    }

    /// Merges another activated library's bindings into this set.
    pub fn merge(&mut self, other: LibraryBindings) {
        self.declare_only.extend(other.declare_only);
        self.library_files.extend(other.library_files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_declare_only_when_case_differs_then_resolves() {
        let mut bindings = LibraryBindings::new();
        bindings.insert_declare_only("LREAL_TO_FMTSTR", "Tc2_Utilities");
        assert_eq!(
            bindings.get_declare_only("lreal_to_fmtstr"),
            Some("Tc2_Utilities")
        );
        assert_eq!(
            bindings.get_declare_only("LREAL_TO_FMTSTR"),
            Some("Tc2_Utilities")
        );
        assert_eq!(bindings.get_declare_only("OTHER"), None);
    }

    #[test]
    fn is_library_file_when_registered_then_true() {
        let mut bindings = LibraryBindings::new();
        let file = FileId::from_string("Tc2_Utilities.st");
        bindings.add_library_file(file.clone());
        assert!(bindings.is_library_file(&file));
        assert!(!bindings.is_library_file(&FileId::from_string("user.st")));
    }

    #[test]
    fn merge_when_two_libraries_then_union() {
        let mut a = LibraryBindings::new();
        a.insert_declare_only("F_A", "LibA");
        a.add_library_file(FileId::from_string("a.st"));

        let mut b = LibraryBindings::new();
        b.insert_declare_only("F_B", "LibB");
        b.add_library_file(FileId::from_string("b.st"));

        a.merge(b);
        assert_eq!(a.get_declare_only("F_A"), Some("LibA"));
        assert_eq!(a.get_declare_only("F_B"), Some("LibB"));
        assert!(a.is_library_file(&FileId::from_string("a.st")));
        assert!(a.is_library_file(&FileId::from_string("b.st")));
    }

    #[test]
    fn default_is_empty() {
        let bindings = LibraryBindings::default();
        assert!(bindings.is_empty());
    }
}
