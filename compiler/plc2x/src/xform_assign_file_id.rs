//! Transform that assigns the file ID into a library.
//!
//! The parser does not have a way to track the file ID
//! while parsing, so this transform sets the file ID
//! after parsing.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{FileId, SourceLoc};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;

pub fn apply(lib: Library, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut transform = TransformFileId { file_id };
    transform.fold_library(lib)
}

struct TransformFileId<'a> {
    file_id: &'a FileId,
}

impl<'a> Fold<Diagnostic> for TransformFileId<'a> {
    fn fold_source_loc(&mut self, node: SourceLoc) -> Result<SourceLoc, Diagnostic> {
        Ok(SourceLoc {
            start: node.start,
            end: node.end,
            file_id: self.file_id.clone(),
        })
    }
}
