//! Member qualifiers on function blocks (OOP extension), such as the
//! `ABSTRACT` in `FUNCTION_BLOCK ABSTRACT FB_Base`.
//!
//! See `specs/design/beckhoff-twincat-dialect.md` §1.5.

use crate::core::SourceSpan;

/// The kind of a single qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberQualifierKind {
    Abstract,
}

/// One qualifier as written in the source.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberQualifier {
    pub kind: MemberQualifierKind,
    pub span: SourceSpan,
}

/// The qualifiers on a declaration, in source order.
///
/// Source order is kept (rather than a set of flags) so that a later
/// semantic rule can report duplicates and a wrong order at the qualifier
/// that causes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemberQualifiers(Vec<MemberQualifier>);

impl MemberQualifiers {
    pub fn new(qualifiers: Vec<MemberQualifier>) -> Self {
        Self(qualifiers)
    }

    pub fn is_abstract(&self) -> bool {
        self.has(MemberQualifierKind::Abstract)
    }

    fn has(&self, kind: MemberQualifierKind) -> bool {
        self.0.iter().any(|q| q.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualifier(kind: MemberQualifierKind) -> MemberQualifier {
        MemberQualifier {
            kind,
            span: SourceSpan::default(),
        }
    }

    #[test]
    fn is_abstract_when_empty_then_false() {
        assert!(!MemberQualifiers::default().is_abstract());
    }

    #[test]
    fn is_abstract_when_abstract_present_then_true() {
        let qualifiers = MemberQualifiers::new(vec![qualifier(MemberQualifierKind::Abstract)]);
        assert!(qualifiers.is_abstract());
    }
}
