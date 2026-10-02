//! Semantic rule that gates the qualifiers on a function block declaration
//! (e.g. `FUNCTION_BLOCK PUBLIC FINAL FB_Motor`) behind
//! `--allow-fb-inheritance`.
//!
//! The access specifiers, `FINAL` and `OVERRIDE` are contextual words: the
//! parser matches them by text, only between `FUNCTION_BLOCK` and the name,
//! and they stay ordinary identifiers everywhere else. The parser has no
//! access to the compiler options, so it accepts them unconditionally and,
//! per ADR-0040 rule 3, this post-parse rule enforces the flag.
//!
//! Method qualifiers need no rule here: without the flag `METHOD` is not a
//! keyword, so a method cannot be declared at all. `ABSTRACT` on a function
//! block is a demoted keyword without the flag and is already a syntax
//! error, so this rule never sees it in practice; it is still reported if
//! present, so the rule does not depend on how the parser gates it.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! FUNCTION_BLOCK PUBLIC FB_Motor
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::FunctionBlockDeclaration,
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
    if options.allow_fb_inheritance {
        return Ok(());
    }

    run_rule(
        RuleMemberQualifierAllowed {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleMemberQualifierAllowed {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleMemberQualifierAllowed {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleMemberQualifierAllowed {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        if let Some(oop) = &node.oop {
            for qualifier in oop.qualifiers.iter() {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::MemberQualifierNotAllowed,
                        Label::span(qualifier.span.clone(), "Function block qualifier"),
                    )
                    .with_context("qualifier", &qualifier.kind.keyword().to_string())
                    .with_context("function_block", &node.name.to_string()),
                );
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_flag() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
FUNCTION_BLOCK PUBLIC FB_Motor
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK";

    rule_err1!(
        apply_when_fb_qualifier_and_flag_disabled_then_error,
        SOURCE,
        Problem::MemberQualifierNotAllowed
    );

    rule_ok_with!(
        apply_when_fb_qualifier_and_flag_enabled_then_ok,
        opts_flag(),
        SOURCE
    );

    // The label points at the qualifier, not at the whole declaration.
    rule_err1_at!(
        apply_when_fb_qualifier_then_label_names_the_qualifier,
        "
FUNCTION_BLOCK INTERNAL FB_Motor
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK",
        Problem::MemberQualifierNotAllowed,
        "INTERNAL"
    );

    // Each qualifier is reported on its own.
    rule_errn!(
        apply_when_several_fb_qualifiers_then_one_error_each,
        "
FUNCTION_BLOCK PUBLIC FINAL FB_Motor
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK",
        2,
        Problem::MemberQualifierNotAllowed
    );

    // A function block named like a qualifier is standard syntax.
    rule_ok!(
        apply_when_fb_named_like_qualifier_then_never_flagged,
        "
FUNCTION_BLOCK Internal
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK"
    );

    // Qualifier words used as variable names are standard syntax.
    rule_ok!(
        apply_when_qualifier_words_are_variables_then_never_flagged,
        "
FUNCTION_BLOCK FB_Motor
VAR
    Public : INT;
    Final : INT;
END_VAR
Final := Public;
END_FUNCTION_BLOCK"
    );
}
