//! Common items useful for working with IEC 61131-3 elements but not
//! part of the standard.
use core::fmt;
use std::ops::{Deref, Range};
use std::path::Path;
use std::str::FromStr;
use std::{cmp::Ordering, hash::Hash, hash::Hasher};

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

/// FileId is an identifier for a file (may be local or remote).
///
/// FileId is normally useful in the context of source positions
/// where a source position is in a file.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FileId(String);

impl FileId {
    /// Creates an empty file identifier.
    pub fn new() -> Self {
        FileId::default()
    }

    /// Creates a file identifier from the path.
    pub fn from_path(path: &Path) -> Self {
        FileId(String::from(path.to_string_lossy().deref()))
    }

    /// Creates a file identifier from the slice. The slice
    /// is normally the file path.
    pub fn from_string(path: &str) -> Self {
        FileId(String::from(path))
    }
}

/// Location in a file of a language element instance.
///
/// The location is defined by indices in the source file.
#[derive(Debug, Clone)]
pub struct SourceLoc {
    pub start: usize,
    pub end: usize,
    pub file_id: FileId,
}

impl SourceLoc {
    pub fn range(start: usize, end: usize) -> SourceLoc {
        SourceLoc {
            start,
            end,
            file_id: FileId::default(),
        }
    }

    pub fn with_file_id(&self, file_id: &FileId) -> Self {
        SourceLoc {
            start: self.start,
            end: self.end,
            file_id: file_id.clone(),
        }
    }
}

impl Default for SourceLoc {
    fn default() -> Self {
        SourceLoc::range(0, 0)
    }
}

impl PartialEq for SourceLoc {
    fn eq(&self, other: &Self) -> bool {
        // Two source locations are equal by default? Yes - when comparing
        // items, we rarely want to know that they were declared at the same
        // position. With this, we can use the derived "Clone" implementation.
        true
    }
}
impl Eq for SourceLoc {}

pub trait SourcePosition {
    /// Get the source code position of the object.
    fn position(&self) -> &SourceLoc;
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
    pub position: SourceLoc,
}

impl Id {
    /// Converts a `&str` into an `Identifier`.
    pub fn from(str: &str) -> Self {
        Id {
            original: String::from(str),
            lower_case: String::from(str).to_lowercase(),
            position: SourceLoc::default(),
        }
    }

    pub fn with_position(mut self, loc: SourceLoc) -> Self {
        self.position = loc;
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
        Id::from(self.original.as_str()).with_position(self.position.clone())
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

impl SourcePosition for Id {
    fn position(&self) -> &SourceLoc {
        &self.position
    }
}
