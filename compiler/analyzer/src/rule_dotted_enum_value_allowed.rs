//! Semantic rule that gates the `Type.value` spelling of an enumerated value
//! (`E_Blink.slow`) behind `--allow-dotted-enum-value`.
//!
//! IEC 61131-3 writes a qualified enumerated value `Type#value`. TwinCAT and
//! CODESYS write `Type.value`. The parser has no access to the compiler
//! options, so it accepts both spellings and records which one was written
//! (`EnumeratedValue::dotted`); per ADR-0040 rule 3, this post-parse rule
//! enforces the flag. A value written without a type, and a dotted name that
//! resolves to a constant or a structure member, are standard and never
//! flagged.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     s : E_Blink;
//! END_VAR
//!     CASE s OF
//!         E_Blink.slow: ;
//!     END_CASE;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::EnumeratedValue,
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
    if options.allow_dotted_enum_value {
        return Ok(());
    }

    run_rule(
        RuleDottedEnumValueAllowed {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleDottedEnumValueAllowed {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleDottedEnumValueAllowed {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleDottedEnumValueAllowed {
    type Value = ();

    fn visit_enumerated_value(
        &mut self,
        node: &EnumeratedValue,
    ) -> Result<Self::Value, Infallible> {
        if node.dotted {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::DottedEnumValueNotAllowed,
                    Label::span(node.value.span.clone(), "Enumerated value"),
                )
                .with_context("value", &node.value.to_string()),
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
            allow_dotted_enum_value: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
TYPE E_Blink : (off, slow, fast); END_TYPE

PROGRAM main
VAR
    s : E_Blink;
END_VAR
    CASE s OF
        E_Blink.slow: ;
    END_CASE;
END_PROGRAM";

    rule_err!(
        apply_when_dotted_enum_value_and_flag_disabled_then_error,
        SOURCE,
        [Problem::DottedEnumValueNotAllowed]
    );

    rule_ok!(
        apply_when_dotted_enum_value_and_flag_enabled_then_ok,
        SOURCE,
        opts_flag()
    );

    rule_ok!(
        apply_when_enum_value_uses_hash_then_never_flagged,
        "
TYPE E_Blink : (off, slow, fast); END_TYPE

PROGRAM main
VAR
    s : E_Blink;
END_VAR
    CASE s OF
        E_Blink#slow: ;
    END_CASE;
END_PROGRAM"
    );
}
