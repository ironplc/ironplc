//! Transform that assigns the file ID into a library.
//!
//! The parser does not have a way to track the file ID
//! while parsing, so this transform sets the file ID
//! after parsing.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{FileId, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;

pub fn apply(lib: Library, file_id: &FileId) -> Result<Library, Diagnostic> {
    let mut transform = TransformFileId { file_id };
    transform.fold_library(lib)
}

struct TransformFileId<'a> {
    file_id: &'a FileId,
}

impl Fold<Diagnostic> for TransformFileId<'_> {
    fn fold_source_span(&mut self, node: SourceSpan) -> Result<SourceSpan, Diagnostic> {
        Ok(SourceSpan {
            start: node.start,
            end: node.end,
            file_id: self.file_id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use dsl::core::Located;
    use ironplc_dsl::{
        common::{DataTypeDeclarationKind, LibraryElementKind},
        core::FileId,
    };

    use crate::{options::ParseOptions, parse_program};

    use super::apply;

    #[test]
    fn apply_when_source_loc_then_changes_value() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE
                ";

        let input = parse_program(
            program,
            &FileId::from_string("input"),
            &ParseOptions::default(),
        )
        .unwrap();
        let expected_fid = FileId::from_string("output");
        let library = apply(input, &expected_fid).unwrap();

        let data_type = match library.elements.first().unwrap() {
            LibraryElementKind::DataTypeDeclaration(dt) => dt,
            _ => panic!(),
        };

        let enum_type = match data_type {
            DataTypeDeclarationKind::Enumeration(enum_data_type) => enum_data_type,
            _ => panic!(),
        };

        let span = enum_type.type_name.span();

        assert_eq!(6, span.start);
        assert_eq!(11, span.end);
        assert_eq!(expected_fid, span.file_id);
    }
}
