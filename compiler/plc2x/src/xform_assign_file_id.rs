use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::{Fold, Foldable};

pub fn apply(lib: Library, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut transform = TransformFileId { file_id };
    transform.fold(lib)
}

struct TransformFileId<'a> {
    file_id: &'a FileId,
}

impl<'a> Fold<Diagnostic> for TransformFileId<'a> {
    fn fold_variable_declaration(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        Ok(VarDecl {
            identifier: node.identifier,
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Foldable::fold(node.initializer, self)?,
            position: node.position.with_file_id(self.file_id),
        })
    }
}
