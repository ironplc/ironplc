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
    use ironplc_dsl::core::FileId;

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

        // Use a more direct approach: collect all spans and verify they have the expected FileId
        // This avoids the need for complex pattern matching
        use ironplc_dsl::fold::Fold;

        let mut spans = Vec::new();
        struct SpanCollector<'a> {
            spans: &'a mut Vec<ironplc_dsl::core::SourceSpan>,
        }

        impl Fold<()> for SpanCollector<'_> {
            fn fold_source_span(
                &mut self,
                node: ironplc_dsl::core::SourceSpan,
            ) -> Result<ironplc_dsl::core::SourceSpan, ()> {
                self.spans.push(node.clone());
                Ok(node)
            }
        }

        let mut collector = SpanCollector { spans: &mut spans };
        let _ = collector.fold_library(library);

        // Find the span that matches our expected position (the type name "LEVEL")
        let type_name_span = spans
            .iter()
            .find(|span| span.start == 6 && span.end == 11)
            .expect("Should find the type name span");

        assert_eq!(expected_fid, type_name_span.file_id);
    }

    #[test]
    fn apply_assigns_same_file_id_to_all_spans() {
        use ironplc_dsl::fold::Fold;

        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
OTHER : (A, B, C) := A;
END_TYPE
                ";

        let input = parse_program(
            program,
            &FileId::from_string("input"),
            &ParseOptions::default(),
        )
        .unwrap();
        let expected_fid = FileId::from_string("shared_file.rs");
        let library = apply(input, &expected_fid).unwrap();

        // Collect all FileIds from the AST
        let mut file_ids = Vec::new();
        struct FileIdCollector<'a> {
            file_ids: &'a mut Vec<FileId>,
        }

        impl Fold<()> for FileIdCollector<'_> {
            fn fold_source_span(
                &mut self,
                node: ironplc_dsl::core::SourceSpan,
            ) -> Result<ironplc_dsl::core::SourceSpan, ()> {
                self.file_ids.push(node.file_id.clone());
                Ok(node)
            }
        }

        let mut collector = FileIdCollector {
            file_ids: &mut file_ids,
        };
        let _ = collector.fold_library(library);

        // Verify all FileIds are equal (the Arc sharing is tested in the dsl crate)
        assert!(!file_ids.is_empty(), "Should have collected some FileIds");

        for file_id in &file_ids {
            assert_eq!(*file_id, expected_fid);
        }
    }
}
