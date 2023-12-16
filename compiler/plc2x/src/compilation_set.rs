use ironplc_dsl::{common::Library, core::FileId};

/// A source that can be compiled together with other items.
pub enum CompilationSource<'a> {
    /// A parsed library. The library should be parsed but not linked.
    Library(Library),
    /// A text string from the specified file.
    Text((String, FileId)),
    /// A text string from the specified file.
    TextRef((&'a String, FileId)),
}

/// A set of sources that should be compiled together.
pub struct CompilationSet<'a> {
    // TODO make these references so that we don't clone unnecessarily
    pub sources: Vec<CompilationSource<'a>>,

    pub references: Vec<&'a CompilationSource<'a>>,
}

impl<'a> CompilationSet<'a> {
    /// Initializes a new compilation set with no content.
    pub fn new() -> Self {
        Self { sources: vec![], references: vec![] }
    }

    /// Initializes a new compilation set with the library as the initial content.
    pub fn of(library: Library) -> Self {
        Self {
            sources: vec![CompilationSource::Library(library)],
            references: vec![],
        }
    }

    /// Appends an compilation source to the back of a set.
    pub fn push(&mut self, source: CompilationSource<'a>) {
        self.sources.push(source);
    }

    /// Appends an compilation source to the back of a set.
    pub fn push_source_into(&mut self, source: String, file_id: FileId) {
        self.sources.push(CompilationSource::Text((source, file_id)));
    }

    pub fn push_source(&mut self, source: &'a String, file_id: FileId) {
        self.sources.push(CompilationSource::TextRef((source, file_id)));
    }

    pub fn content(&self, file_id: &FileId) -> Option<&String> {
        for source in &self.sources {
            match source {
                CompilationSource::Library(_lib) => {},
                CompilationSource::Text(txt) => {
                    if txt.1 == *file_id {
                        return Some(&txt.0);
                    }
                },
                CompilationSource::TextRef(txt) => {
                    if txt.1 == *file_id {
                        return Some(&txt.0);
                    }
                },
            }
        }
        
        for source in &self.references {
            match source {
                CompilationSource::Library(_lib) => {},
                CompilationSource::Text(txt) => {
                    if txt.1 == *file_id {
                        return Some(&txt.0);
                    }
                },
                CompilationSource::TextRef(txt) => {
                    if txt.1 == *file_id {
                        return Some(&txt.0);
                    }
                },
            }
        }
        
        None
    }
}

impl<'a> Default for CompilationSet<'a> {
    fn default() -> Self {
        Self::new()
    }
}
