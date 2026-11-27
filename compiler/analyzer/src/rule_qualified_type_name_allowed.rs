//! Semantic rule that gates a library prefix on a type name
//! (`TcUnit.FB_TestSuite`, `BROTLib.E_Blink`) behind
//! `--allow-qualified-type-names`.
//!
//! IEC 61131-3 Edition 2 names a type by its bare identifier; TwinCAT and
//! CODESYS let a name carry the library it comes from. The parser has no
//! access to the compiler options, so it accepts the prefix unconditionally
//! and, per ADR-0040 rule 3, this post-parse rule enforces the flag. Whether
//! the prefix names the library that declares the type is
//! `rule_qualified_type_name_library`'s to report.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Test EXTENDS TcUnit.FB_TestSuite
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::TypeName,
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
    if options.allow_qualified_type_names {
        return Ok(());
    }

    run_rule(
        RuleQualifiedTypeNameAllowed {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleQualifiedTypeNameAllowed {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleQualifiedTypeNameAllowed {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleQualifiedTypeNameAllowed {
    type Value = ();

    fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, Infallible> {
        if let Some(prefix) = &node.qualifier {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::QualifiedTypeNameNotAllowed,
                    Label::span(prefix.span.clone(), "Library prefix"),
                )
                .with_context_id("type", &node.id),
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
            allow_qualified_type_names: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
FUNCTION_BLOCK FB_Test
VAR
    suite : TcUnit.FB_TestSuite;
END_VAR
END_FUNCTION_BLOCK";

    rule_err!(
        apply_when_qualified_type_name_and_flag_disabled_then_error,
        SOURCE,
        [Problem::QualifiedTypeNameNotAllowed]
    );

    rule_ok!(
        apply_when_qualified_type_name_and_flag_enabled_then_ok,
        SOURCE,
        opts_flag()
    );

    // The label points at the prefix, not at the whole name.
    rule_err_at!(
        apply_when_qualified_variable_type_then_label_names_the_prefix,
        "
PROGRAM main
VAR
    x : BROTLib.E_Blink;
END_VAR
END_PROGRAM",
        Problem::QualifiedTypeNameNotAllowed,
        "BROTLib"
    );

    // The type of a `Lib.Type.value` carries the prefix as well.
    rule_err!(
        apply_when_dotted_value_has_library_prefix_then_error,
        "
PROGRAM main
VAR
    s : INT;
END_VAR
    CASE s OF
        BROTLib.E_Blink.slow: ;
    END_CASE;
END_PROGRAM",
        [Problem::QualifiedTypeNameNotAllowed]
    );

    rule_ok!(
        apply_when_type_name_has_no_prefix_then_never_flagged,
        "
PROGRAM main
VAR
    x : INT;
END_VAR
END_PROGRAM"
    );
}
