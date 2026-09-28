//! Member qualifiers on methods and function blocks (OOP extension), such
//! as the `PRIVATE` in `METHOD PRIVATE Reset` or the `ABSTRACT` in
//! `FUNCTION_BLOCK ABSTRACT FB_Base`.
//!
//! Qualifiers are metadata only: access is not enforced (ADR-0041).
//!
//! See `specs/design/beckhoff-twincat-dialect.md` §1.5.

use crate::core::SourceSpan;

/// Who may call a method or use a function block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessSpecifier {
    Public,
    Private,
    Protected,
    Internal,
}

/// The kind of a single qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberQualifierKind {
    Access(AccessSpecifier),
    Abstract,
    Final,
    Override,
}

impl MemberQualifierKind {
    /// The keyword as written in source, in upper case.
    pub fn keyword(&self) -> &'static str {
        match self {
            MemberQualifierKind::Access(AccessSpecifier::Public) => "PUBLIC",
            MemberQualifierKind::Access(AccessSpecifier::Private) => "PRIVATE",
            MemberQualifierKind::Access(AccessSpecifier::Protected) => "PROTECTED",
            MemberQualifierKind::Access(AccessSpecifier::Internal) => "INTERNAL",
            MemberQualifierKind::Abstract => "ABSTRACT",
            MemberQualifierKind::Final => "FINAL",
            MemberQualifierKind::Override => "OVERRIDE",
        }
    }
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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemberQualifier> {
        self.0.iter()
    }

    /// The first access specifier, if any.
    pub fn access(&self) -> Option<AccessSpecifier> {
        self.0.iter().find_map(|q| match q.kind {
            MemberQualifierKind::Access(access) => Some(access),
            _ => None,
        })
    }

    pub fn is_abstract(&self) -> bool {
        self.has(MemberQualifierKind::Abstract)
    }

    pub fn is_final(&self) -> bool {
        self.has(MemberQualifierKind::Final)
    }

    pub fn is_override(&self) -> bool {
        self.has(MemberQualifierKind::Override)
    }

    fn has(&self, kind: MemberQualifierKind) -> bool {
        self.0.iter().any(|q| q.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualifiers(kinds: &[MemberQualifierKind]) -> MemberQualifiers {
        MemberQualifiers::new(
            kinds
                .iter()
                .map(|kind| MemberQualifier {
                    kind: *kind,
                    span: SourceSpan::default(),
                })
                .collect(),
        )
    }

    #[test]
    fn is_abstract_when_empty_then_false() {
        assert!(!MemberQualifiers::default().is_abstract());
    }

    #[test]
    fn is_abstract_when_abstract_present_then_true() {
        assert!(qualifiers(&[MemberQualifierKind::Abstract]).is_abstract());
    }

    #[test]
    fn access_when_no_access_specifier_then_none() {
        assert_eq!(qualifiers(&[MemberQualifierKind::Final]).access(), None);
    }

    #[test]
    fn access_when_access_specifier_after_final_then_found() {
        let q = qualifiers(&[
            MemberQualifierKind::Final,
            MemberQualifierKind::Access(AccessSpecifier::Private),
        ]);
        assert_eq!(q.access(), Some(AccessSpecifier::Private));
    }

    #[test]
    fn is_final_and_is_override_when_present_then_true() {
        let q = qualifiers(&[MemberQualifierKind::Final, MemberQualifierKind::Override]);
        assert!(q.is_final());
        assert!(q.is_override());
        assert!(!q.is_abstract());
    }

    #[test]
    fn keyword_when_access_specifier_then_upper_case_word() {
        assert_eq!(
            MemberQualifierKind::Access(AccessSpecifier::Protected).keyword(),
            "PROTECTED"
        );
    }
}
