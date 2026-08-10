//! Compatibility-library POU bindings.
//!
//! A binding maps a library POU to a non-default implementation: a native VM
//! builtin (reached exclusively through the library's manifest — the builtin
//! adds no name to any scope) or the declare-only state (the signature exists,
//! calling it is a compile error). See
//! `specs/design/compatibility-library-format.md` §Bindings and ADR-0042.
//!
//! Binding information travels *out of band*: the analyze merge erases which
//! POUs came from a library, and provenance markers in the AST are forbidden,
//! so the loader produces this side-table and the driver threads it into
//! codegen. The analyzer never sees bindings — which is exactly what makes a
//! declare-only call pass `check`.

use std::collections::{HashMap, HashSet};

use crate::core::FileId;

/// How a bound library POU is implemented, from the manifest's per-version
/// bindings table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PouBinding {
    /// Calls lower to the named native VM builtin (an internal compiler/VM
    /// identifier such as `sqrt_lreal`, resolved to a func_id at codegen —
    /// never a callable name in any scope).
    Intrinsic {
        /// The manifest's intrinsic name.
        name: String,
    },
    /// The POU's signature exists so the library surface can land ahead of
    /// its implementation; a *call* is a compile error (P4046).
    DeclareOnly,
}

/// A binding plus the context needed to act on it at codegen: which library
/// declared it (named by the P4046 diagnostic) and the manifest file to
/// anchor packaging errors on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundPou {
    /// The declaring library's name (e.g. `Tc2_Math`).
    pub library: String,
    /// The library's manifest file, anchoring diagnostics about the binding
    /// itself (e.g. an unresolvable intrinsic name).
    pub manifest_file: FileId,
    /// The binding.
    pub binding: PouBinding,
}

/// The bindings of every activated library, keyed by uppercased POU name,
/// plus the set of library source files.
///
/// The file set is what preserves user shadowing: codegen skips a bound
/// `FunctionDeclaration` only when its `FileId` is a library file, so a
/// user-defined POU with the same name still compiles as the user's function.
///
/// `Default` is the empty set, so codegen consumers that never activate a
/// library (benchmarks, direct `compile()` callers) are unaffected and an
/// intrinsic-bound call in an unthreaded consumer fails closed rather than
/// lowering wrongly.
#[derive(Debug, Clone, Default)]
pub struct LibraryBindings {
    /// Uppercased POU name → binding.
    bindings: HashMap<String, BoundPou>,
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
        self.bindings.is_empty() && self.library_files.is_empty()
    }

    /// Registers a binding for a POU name (matched case-insensitively via
    /// uppercasing, like the function environment).
    pub fn insert(&mut self, pou_name: &str, bound: BoundPou) {
        self.bindings.insert(pou_name.to_uppercase(), bound);
    }

    /// Looks up the binding for a POU name (case-insensitive).
    pub fn get(&self, pou_name: &str) -> Option<&BoundPou> {
        self.bindings.get(&pou_name.to_uppercase())
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
        self.bindings.extend(other.bindings);
        self.library_files.extend(other.library_files);
    }

    /// Iterates over every registered `(uppercased POU name, binding)` pair.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &BoundPou)> {
        self.bindings.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(library: &str, binding: PouBinding) -> BoundPou {
        BoundPou {
            library: library.to_string(),
            manifest_file: FileId::from_string("library.toml"),
            binding,
        }
    }

    #[test]
    fn get_when_case_differs_then_resolves() {
        let mut bindings = LibraryBindings::new();
        bindings.insert(
            "MY_SQRT",
            bound(
                "Tc2_Math",
                PouBinding::Intrinsic {
                    name: "sqrt_lreal".to_string(),
                },
            ),
        );
        assert!(bindings.get("my_sqrt").is_some());
        assert!(bindings.get("MY_SQRT").is_some());
        assert!(bindings.get("MY_ABS").is_none());
    }

    #[test]
    fn is_library_file_when_registered_then_true() {
        let mut bindings = LibraryBindings::new();
        let file = FileId::from_string("Tc2_Math.st");
        bindings.add_library_file(file.clone());
        assert!(bindings.is_library_file(&file));
        assert!(!bindings.is_library_file(&FileId::from_string("user.st")));
    }

    #[test]
    fn merge_when_two_libraries_then_union() {
        let mut a = LibraryBindings::new();
        a.insert("MY_SQRT", bound("Tc2_Math", PouBinding::DeclareOnly));
        a.add_library_file(FileId::from_string("a.st"));

        let mut b = LibraryBindings::new();
        b.insert("MY_ABS", bound("Tc2_Utilities", PouBinding::DeclareOnly));
        b.add_library_file(FileId::from_string("b.st"));

        a.merge(b);
        assert!(a.get("MY_SQRT").is_some());
        assert!(a.get("MY_ABS").is_some());
        assert!(a.is_library_file(&FileId::from_string("a.st")));
        assert!(a.is_library_file(&FileId::from_string("b.st")));
    }

    #[test]
    fn default_is_empty() {
        let bindings = LibraryBindings::default();
        assert!(bindings.is_empty());
        assert_eq!(bindings.iter().count(), 0);
    }
}
