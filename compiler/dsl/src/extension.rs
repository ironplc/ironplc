//! Marks AST nodes representing non-standard language extensions
//! that IronPLC parses but does not yet semantically analyze.
//!
//! See `specs/design/beckhoff-twincat-dialect.md` and
//! `specs/plans/2026-07-18-twincat-extends-implements-interface.md`.

use crate::core::SourceSpan;

/// Marker trait for AST nodes representing non-standard language
/// extensions.
///
/// Nodes implementing this trait are parsed and represented in the AST but
/// not yet semantically analyzed or supported in code generation. The
/// semantic rule `rule_unsupported_extension` walks the AST and emits P9999
/// for every node that implements this trait.
///
/// As each extension graduates to full support, remove its `LanguageExtension`
/// impl. The semantic rule automatically stops flagging it.
pub trait LanguageExtension {
    /// Human-readable name of this extension (e.g., "EXTENDS clause").
    fn extension_name(&self) -> &'static str;

    /// The source span for diagnostic reporting.
    fn extension_span(&self) -> SourceSpan;
}
