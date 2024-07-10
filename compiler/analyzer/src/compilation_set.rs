//! Provides a view onto a set of files that are compiled together
//! as a single unit.
use ironplc_dsl::{common::Library, core::FileId};

/// A source that can be compiled together with other items.
pub enum CompilationSource {
    /// A parsed library. The library should be parsed but not linked.
    Library(Library),
    /// A text string from the specified file.
    Text((String, FileId)),
}

/// A set of sources that should be compiled together.
pub struct CompilationSet {
    // TODO make these references so that we don't clone unnecessarily
    pub sources: Vec<CompilationSource>,
}

impl CompilationSet {
    /// Initializes a new compilation set with no content.
    pub fn new() -> Self {
        Self { sources: vec![] }
    }

    /// Initializes a new compilation set with the library as the initial content.
    pub fn of(library: Library) -> Self {
        Self {
            sources: vec![CompilationSource::Library(library)],
        }
    }

    /// Appends an compilation source to the back of a set.
    pub fn push(&mut self, source: CompilationSource) {
        self.sources.push(source);
    }

    pub fn find(&self, file_id: &FileId) -> Option<&CompilationSource> {
        for src in self.sources.iter() {
            match src {
                CompilationSource::Library(_lib) => {}
                CompilationSource::Text(txt) => {
                    if txt.1 == *file_id {
                        return Some(src);
                    }
                }
            }
        }
        None
    }
}

impl Default for CompilationSet {
    fn default() -> Self {
        Self::new()
    }
}
