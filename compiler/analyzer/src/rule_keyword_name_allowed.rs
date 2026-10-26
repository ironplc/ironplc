//! Semantic rule that gates using one of the keywords `ON`, `STEP`, `R_EDGE`
//! or `F_EDGE` as a name (e.g. `TYPE M : (Off, On); END_TYPE`) behind
//! `--allow-keywords-as-names`.
//!
//! IEC 61131-3 reserves its keywords: they "shall not be used for any other
//! purpose, e.g., variable names" (2003, 2.1.3; 2013, 6.1.3). Other vendors'
//! toolchains do not reserve these four, so programs written for them name
//! variables and enumeration members with them; which dialects enable the
//! extension is defined by the dialect mapping in `options.rs`, not restated
//! here.
//!
//! Per ADR-0040 the grammar recognizes the superset unconditionally: each of
//! these keywords keeps its keyword role (`RESOURCE r ON PLC`, an SFC
//! `STEP`, an `R_EDGE` input) in the same program that may use it as a name,
//! so there is no token to demote or reject pre-parse. The parser's
//! `variable_identifier` accepts the four as names, and this rule is what
//! enforces the flag.
//!
//! The lexer never produces an identifier spelled as a keyword, so a name
//! spelled as one of the four can only have come from that carve-out, and
//! a visit over every [`Id`] finds each use without listing the positions
//! that take a name.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! TYPE
//!     M : (Off, On);
//! END_TYPE
//! ```
use ironplc_dsl::{
    core::{Id, Located},
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

/// The keywords the parser also accepts as names, in lower case.
const KEYWORDS_ACCEPTED_AS_NAMES: [&str; 4] = ["on", "step", "r_edge", "f_edge"];

pub fn apply(
    lib: &ironplc_dsl::common::Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_keywords_as_names {
        return Ok(());
    }

    run_rule(
        RuleKeywordName {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleKeywordName {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleKeywordName {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleKeywordName {
    type Value = ();

    fn visit_id(&mut self, node: &Id) -> Result<Self::Value, Infallible> {
        if KEYWORDS_ACCEPTED_AS_NAMES.contains(&node.lower_case().as_str()) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::KeywordAsNameNotAllowed,
                Label::span(node.span(), "Keyword used as a name"),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_flag() -> CompilerOptions {
        CompilerOptions {
            allow_keywords_as_names: true,
            ..CompilerOptions::default()
        }
    }

    const SOURCE: &str = "
TYPE
M : (Off, On);
END_TYPE
PROGRAM main
VAR
    c : M;
END_VAR
    c := On;
END_PROGRAM";

    // The declaration and the use are each reported.
    rule_err!(
        apply_when_keyword_names_member_and_flag_disabled_then_error,
        SOURCE,
        [Problem::KeywordAsNameNotAllowed; 2]
    );

    rule_ok!(
        apply_when_keyword_names_member_and_flag_enabled_then_ok,
        SOURCE,
        opts_flag()
    );

    #[rstest::rstest]
    #[case::on("On")]
    #[case::step("Step")]
    #[case::r_edge("R_EDGE")]
    #[case::f_edge("f_edge")]
    fn apply_when_keyword_names_variable_and_flag_disabled_then_error_at_name(#[case] name: &str) {
        let program = format!(
            "
FUNCTION_BLOCK fb
VAR
    {name} : BOOL;
END_VAR
END_FUNCTION_BLOCK"
        );
        let diagnostics =
            crate::test_helpers::rule_diagnostics(apply, &program, &CompilerOptions::default());
        assert_eq!(
            crate::test_helpers::diagnostic_codes(&diagnostics),
            [Problem::KeywordAsNameNotAllowed.code()]
        );
        let location = &diagnostics[0].primary.location;
        assert_eq!(name, &program[location.start..location.end]);
    }

    // The keywords in their keyword role are not names and are never flagged.
    rule_ok!(
        apply_when_edge_qualifier_used_as_keyword_then_never_flagged,
        "
FUNCTION_BLOCK fb
VAR_INPUT
    trigger : BOOL R_EDGE;
    release : BOOL F_EDGE;
END_VAR
END_FUNCTION_BLOCK"
    );

    // A name that merely contains a keyword is an ordinary identifier.
    rule_ok!(
        apply_when_name_contains_keyword_then_never_flagged,
        "
TYPE
M : (Off, OnState, StepOne);
END_TYPE"
    );
}
