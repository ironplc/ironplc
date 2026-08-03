//! Marks AST nodes representing vendor-specific language extensions that
//! IronPLC parses but does not yet semantically analyze.
//!
//! See `specs/design/beckhoff-twincat-dialect.md` and
//! `specs/plans/2026-07-18-twincat-extends-implements-interface.md`.

use crate::core::SourceSpan;

/// Identifies the vendor or standards origin of a language extension.
///
/// A single extension may have multiple origins (e.g. a construct shared by
/// Beckhoff/CODESYS and Siemens SCL would list both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtensionOrigin {
    /// Beckhoff TwinCAT / CODESYS OOP and type system extensions.
    BeckhoffCodesys,
}

impl ExtensionOrigin {
    /// A human-readable label suitable for diagnostic messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExtensionOrigin::BeckhoffCodesys => "Beckhoff/CODESYS",
        }
    }
}

/// Marker trait for AST nodes representing vendor-specific language
/// extensions.
///
/// Nodes implementing this trait are parsed and represented in the AST but
/// not yet semantically analyzed or supported in code generation. The
/// semantic rule `rule_unsupported_extension` walks the AST and emits P9004
/// for every node that implements this trait.
///
/// As each extension graduates to full support, remove its `VendorExtension`
/// impl. The semantic rule automatically stops flagging it.
pub trait VendorExtension {
    /// Human-readable name of this extension (e.g., "EXTENDS clause").
    fn extension_name(&self) -> &'static str;

    /// Which vendor dialects introduced this extension. A single extension
    /// may originate from multiple vendors.
    fn extension_origins(&self) -> &'static [ExtensionOrigin];

    /// The source span for diagnostic reporting.
    fn extension_span(&self) -> SourceSpan;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_origin_as_str_when_beckhoff_codesys_then_readable_label() {
        assert_eq!(
            ExtensionOrigin::BeckhoffCodesys.as_str(),
            "Beckhoff/CODESYS"
        );
    }
}
