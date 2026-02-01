//! Common items useful for working with IEC 61131-3 elements but not
//! part of the standard.
use core::fmt;
use std::fs::DirEntry;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::{hash::Hash, hash::Hasher};

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

// Static singletons for common FileId values to avoid repeated allocations.
// This is particularly beneficial for test code which frequently uses FileId::default(),
// and for any other commonly used file paths.
static EMPTY_FILE_ID: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(""));

/// FileId identifies the origin of source code.
///
/// FileId is normally useful in the context of source positions
/// where a source position is in a file. It can also represent
/// built-in types that are part of the compiler (e.g., standard
/// library function blocks, elementary types).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FileId {
    /// Source code from a file (local or remote). The string is the file path.
    File(Arc<str>),
    /// Built-in to the compiler (stdlib types, elementary types).
    /// These types have no source file - they are intrinsic to the language.
    BuiltIn,
}

impl FileId {
    /// Creates an empty file identifier.
    pub fn new() -> Self {
        FileId::default()
    }

    /// Creates a file identifier from the path.
    pub fn from_path(path: &Path) -> Self {
        FileId::File(Arc::from(path.to_string_lossy().as_ref()))
    }

    /// Creates a file identifier from the directory entry.
    pub fn from_dir_entry(entry: DirEntry) -> Self {
        FileId::File(Arc::from(entry.path().to_string_lossy().as_ref()))
    }

    /// Creates a file identifier from the slice. The slice
    /// is normally the file path.
    pub fn from_string(path: &str) -> Self {
        FileId::File(Arc::from(path))
    }

    /// Creates a file identifier for built-in types (stdlib, elementary types).
    pub fn builtin() -> Self {
        FileId::BuiltIn
    }

    /// Returns true if this FileId represents a built-in type.
    pub fn is_builtin(&self) -> bool {
        matches!(self, FileId::BuiltIn)
    }

    /// Test-only method to check if two FileIds share the same Arc memory.
    /// This is used to verify that our optimization is working correctly.
    #[cfg(test)]
    pub fn shares_arc_with(&self, other: &FileId) -> bool {
        match (self, other) {
            (FileId::File(a), FileId::File(b)) => Arc::ptr_eq(a, b),
            (FileId::BuiltIn, FileId::BuiltIn) => true,
            _ => false,
        }
    }
}

impl Default for FileId {
    fn default() -> Self {
        FileId::File(EMPTY_FILE_ID.clone())
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileId::File(path) => write!(f, "{}", path),
            FileId::BuiltIn => write!(f, "<builtin>"),
        }
    }
}

/// Location in a file of a language element instance.
///
/// The location is defined by indices in the source file.
#[derive(Debug, Clone)]
pub struct SourceSpan {
    /// The position of the starting character (0-indexed).
    pub start: usize,
    /// The position of the ending character (0-indexed).
    ///
    /// Equals the start position for a length of 1 character.
    pub end: usize,
    pub file_id: FileId,
}

impl SourceSpan {
    pub fn join(start: &SourceSpan, end: &SourceSpan) -> Self {
        Self {
            start: start.start,
            end: end.end,
            file_id: start.file_id.clone(),
        }
    }
    pub fn join2(start: &dyn Located, end: &dyn Located) -> Self {
        Self {
            start: start.span().start,
            end: end.span().start,
            file_id: start.span().file_id.clone(),
        }
    }
    pub fn range(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            file_id: FileId::default(),
        }
    }
    pub fn with_file_id(&self, file_id: &FileId) -> Self {
        Self {
            start: self.start,
            end: self.end,
            file_id: file_id.clone(),
        }
    }

    /// Creates a SourceSpan for built-in types (stdlib, elementary types).
    /// These have no meaningful source position since they are intrinsic to the language.
    pub fn builtin() -> Self {
        Self {
            start: 0,
            end: 0,
            file_id: FileId::builtin(),
        }
    }

    /// Returns true if this span represents a built-in type.
    pub fn is_builtin(&self) -> bool {
        self.file_id.is_builtin()
    }
}

impl Default for SourceSpan {
    fn default() -> Self {
        SourceSpan::range(0, 0)
    }
}

impl PartialEq for SourceSpan {
    fn eq(&self, _other: &Self) -> bool {
        // Two source locations are equal by default? Yes - when comparing
        // items, we rarely want to know that they were declared at the same
        // position. With this, we can use the derived "Clone" implementation.
        true
    }
}
impl Eq for SourceSpan {}

/// Defines an element that has a location in source code.
pub trait Located {
    /// Get the source code position of the object.
    fn span(&self) -> SourceSpan;
}

/// Implements Identifier.
///
/// 61131-3 declares that identifiers are case insensitive.
/// This class ensures that we do case insensitive comparisons
/// and can use containers as appropriate.
///
/// See section 2.1.2.
#[derive(Recurse)]
pub struct Id {
    #[recurse(ignore)]
    pub original: String,
    #[recurse(ignore)]
    pub lower_case: String,
    pub span: SourceSpan,
}

impl Id {
    /// Converts a `&str` into an `Identifier`.
    pub fn from(str: &str) -> Self {
        Id {
            original: String::from(str),
            lower_case: String::from(str).to_lowercase(),
            span: SourceSpan::default(),
        }
    }

    pub fn with_position(mut self, loc: SourceSpan) -> Self {
        self.span = loc;
        self
    }

    /// Converts an `Identifier` into a lower case `String`.
    pub fn lower_case(&self) -> &String {
        &self.lower_case
    }

    pub fn original(&self) -> &String {
        &self.original
    }
}

impl Clone for Id {
    fn clone(&self) -> Self {
        Id::from(self.original.as_str()).with_position(self.span.clone())
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.lower_case == other.lower_case
    }
}
impl Eq for Id {}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lower_case.hash(state);
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.original)
    }
}

impl Located for Id {
    fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_id_when_clone_then_same_underlying_data() {
        // Test that FileId instances with the same path share memory
        let path = "src/main.rs";
        let file_id1 = FileId::from_string(path);
        let file_id2 = FileId::from_string(path);
        let file_id3 = file_id1.clone();

        // Verify they're equal
        assert_eq!(file_id1, file_id2);
        assert_eq!(file_id1, file_id3);

        // Verify that cloning doesn't create new string allocations
        // by checking that the Arc pointers are the same
        assert!(file_id1.shares_arc_with(&file_id3));
    }

    #[test]
    fn file_id_when_display_then_returns_value() {
        let file_id = FileId::from_string("test/file.rs");
        assert_eq!(format!("{file_id}"), "test/file.rs");
    }

    #[test]
    fn file_id_from_path_then_creates_path() {
        use std::path::Path;
        let path = Path::new("src/lib.rs");
        let file_id = FileId::from_path(path);
        assert_eq!(format!("{file_id}"), "src/lib.rs");
    }

    #[test]
    fn file_id_when_different_paths_then_different_arcs() {
        // Verify that different file paths don't share memory (as expected)
        let file1 = FileId::from_string("file1.rs");
        let file2 = FileId::from_string("file2.rs");

        // They should be different
        assert_ne!(file1, file2);

        // They should NOT share Arc memory
        assert!(!file1.shares_arc_with(&file2));

        // But clones of each should share memory
        let file1_clone = file1.clone();
        let file2_clone = file2.clone();

        assert!(file1.shares_arc_with(&file1_clone));
        assert!(file2.shares_arc_with(&file2_clone));
        assert!(!file1.shares_arc_with(&file2_clone));
    }

    #[test]
    fn file_id_builtin_when_display_then_returns_builtin_marker() {
        let file_id = FileId::builtin();
        assert_eq!(format!("{file_id}"), "<builtin>");
    }

    #[test]
    fn file_id_builtin_when_is_builtin_then_true() {
        let builtin = FileId::builtin();
        assert!(builtin.is_builtin());
    }

    #[test]
    fn file_id_file_when_is_builtin_then_false() {
        let file = FileId::from_string("test.rs");
        assert!(!file.is_builtin());
    }

    #[test]
    fn file_id_builtin_when_compared_then_equal() {
        let builtin1 = FileId::builtin();
        let builtin2 = FileId::builtin();
        assert_eq!(builtin1, builtin2);
        assert!(builtin1.shares_arc_with(&builtin2));
    }

    #[test]
    fn file_id_builtin_when_compared_to_file_then_not_equal() {
        let builtin = FileId::builtin();
        let file = FileId::from_string("test.rs");
        assert_ne!(builtin, file);
        assert!(!builtin.shares_arc_with(&file));
    }
}
