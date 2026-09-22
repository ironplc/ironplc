//! Semantic rule that gates the base-type suffix on an enumeration
//! declaration (e.g. `TYPE E_Small : (A, B) WORD; END_TYPE`) behind
//! `--allow-enum-base-type`.
//!
//! The suffix names the elementary type the members are stored in, overriding
//! the automatic count/value-based sizing in `intermediates/enumeration.rs`.
//! IEC 61131-3 has no such form -- its `enumerated_specification` (Annex B) is
//! a parenthesized value list or a type name, and nothing may follow it -- so
//! the suffix is a dialect extension; which dialects enable it is defined by
//! the dialect mapping in `options.rs`, not restated here. Note that this is
//! *not* an Edition 3 addition, which is where it differs from its sibling
//! `--allow-enum-explicit-values`: the two often appear together in vendor
//! code but answer different standards questions.
//!
//! Per ADR-0040 the grammar recognizes the superset unconditionally. The
//! elementary type keyword that spells the suffix (`BYTE`, `WORD`, `INT`, ...)
//! is ordinary standard syntax everywhere else, so there is nothing to demote,
//! and a token rule would have to key on the `)`-then-type-keyword adjacency
//! and would fire on malformed input that is no enum declaration at all. The
//! illegality is visible only in the parsed structure, which ADR-0040 rule 3
//! sends to a post-parse check. The parser records the suffix faithfully in
//! `EnumeratedSpecificationInit::underlying_type`, and this rule is what
//! enforces the flag.
//!
//! The label points at the declared type name rather than at the suffix:
//! `ElementaryTypeName` is a fieldless enum and carries no `SourceSpan`.
//! Giving the suffix its own span would thread one through the parser, the
//! renderer, both analyzer transforms, the XML transform and codegen, so it
//! is tracked separately rather than done here.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! TYPE
//!     E_Small : (A, B) WORD;
//! END_TYPE
//! ```
use ironplc_dsl::{
    common::EnumerationDeclaration,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &ironplc_dsl::common::Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_enum_base_type {
        return Ok(());
    }

    run_rule(
        RuleEnumBaseType {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleEnumBaseType {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleEnumBaseType {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleEnumBaseType {
    type Value = ();

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        if let Some(base_type) = &node.spec_init.underlying_type {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::EnumBaseTypeNotAllowed,
                    Label::span(
                        node.type_name.span(),
                        "Enumeration declared with a base-type suffix",
                    ),
                )
                .with_context("base_type", &base_type.to_string()),
            );
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_flag() -> CompilerOptions {
        CompilerOptions {
            allow_enum_base_type: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
TYPE
E_Small : (A, B) WORD;
END_TYPE";

    rule_err1!(
        apply_when_enum_base_type_and_flag_disabled_then_error,
        SOURCE,
        Problem::EnumBaseTypeNotAllowed
    );

    rule_ok_with!(
        apply_when_enum_base_type_and_flag_enabled_then_ok,
        opts_flag(),
        SOURCE
    );

    // The label has to name the declaration that carries the suffix, or the
    // user cannot tell which type to change in a file full of them.
    rule_err1_at!(
        apply_when_enum_base_type_then_label_names_the_declared_type,
        "
TYPE
COLOR : (RED, GREEN, BLUE);
E_Small : (A, B) WORD;
END_TYPE",
        Problem::EnumBaseTypeNotAllowed,
        "E_Small"
    );

    // A declaration with no suffix is standard syntax and is never flagged,
    // whatever the flag says. This is the automatic-sizing path.
    rule_ok!(
        apply_when_enum_has_no_base_type_then_never_flagged,
        "
TYPE
COLOR : (RED, GREEN, BLUE);
END_TYPE"
    );

    // A default value follows the value list in the same position the suffix
    // would take, and is standard syntax. It must not be mistaken for one.
    rule_ok!(
        apply_when_enum_has_default_value_then_never_flagged,
        "
TYPE
COLOR : (RED, GREEN, BLUE) := GREEN;
END_TYPE"
    );

    // The suffix and a default value can appear together; the suffix is still
    // the only part gated, and still reported exactly once.
    rule_err1!(
        apply_when_enum_base_type_with_default_then_error,
        "
TYPE
E_Small : (A, B) WORD := B;
END_TYPE",
        Problem::EnumBaseTypeNotAllowed
    );

    // Each declaration that carries a suffix is reported on its own, so a
    // file with several gets one diagnostic per declaration.
    rule_errn!(
        apply_when_several_enums_have_base_types_then_one_error_each,
        "
TYPE
E_Small : (A, B) WORD;
E_Other : (C, D) BYTE;
END_TYPE",
        2,
        Problem::EnumBaseTypeNotAllowed
    );
}
