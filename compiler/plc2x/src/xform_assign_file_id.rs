//! Transform that assigns the file ID into a library.
//!
//! The parser does not have a way to track the file ID
//! while parsing, so this transform sets the file ID
//! after parsing.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{FileId, Id};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::{Fold, Folder};

pub fn apply(lib: Library, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut transform = TransformFileId { file_id };
    transform.fold_library(lib)
}

struct TransformFileId<'a> {
    file_id: &'a FileId,
}

impl<'a> Fold<Diagnostic> for TransformFileId<'a> {
    fn fold_id(&mut self, node: Id) -> Result<Id, Diagnostic> {
        Ok(Id {
            original: node.original,
            lower_case: node.lower_case,
            position: node.position.with_file_id(self.file_id),
        })
    }

    fn fold_variable_declaration(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        Ok(VarDecl {
            identifier: node.identifier,
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Folder::fold(node.initializer, self)?,
            position: node.position.with_file_id(self.file_id),
        })
    }
}
