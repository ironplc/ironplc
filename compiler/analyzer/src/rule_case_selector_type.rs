//! Semantic rule that a `CASE` selector is an integer or an enumeration.
//!
//! IEC 61131-3 defines the `CASE` statement over an expression that "shall
//! evaluate to a variable of type `ANY_INT` or enumerated data type" (see
//! 3.3.2.3). The labels are integer literals, integer ranges and enumerated
//! values, and the parser already limits them to those. This rule holds the
//! selector to the same set: a signed or unsigned integer, a subrange (which
//! narrows an integer type), or an enumeration.
//!
//! Every other type is reported, whether or not the backend could compare
//! it. A `REAL` has no integer labels to select between. A bit string
//! (`BYTE`, `WORD`, `DWORD`, `LWORD`) is a pattern rather than a magnitude,
//! and the standard does not list it; a program that selects on one compiled
//! before this rule existed, but should not have. The bit-string *label*
//! extension (`--allow-bit-string-case-labels`) is unaffected: it pairs a
//! radix-prefixed label with an integer selector.
//!
//! A selector whose type the analyzer did not resolve, or resolved to a
//! generic category that is not in the type environment (a bare literal,
//! `CASE 1 OF`, is `ANY_INT`), is skipped rather than reported.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!     Mode : (Idle, Fill, Drain);
//!     Pct : INT (0..100);
//! END_TYPE
//! PROGRAM main
//! VAR
//!     step : DINT;
//!     mode : Mode;
//!     level : Pct;
//!     alarm : BOOL;
//! END_VAR
//!     CASE step OF
//!         1: alarm := TRUE;
//!     END_CASE;
//!     CASE mode OF
//!         Fill: alarm := FALSE;
//!     END_CASE;
//!     CASE level OF
//!         90..100: alarm := TRUE;
//!     END_CASE;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     level : REAL;
//!     flags : WORD;
//!     alarm : BOOL;
//! END_VAR
//!     CASE level OF          (* P4052: REAL is not an integer *)
//!         1: alarm := TRUE;
//!     END_CASE;
//!     CASE flags OF          (* P4052: WORD is a bit string *)
//!         1: alarm := TRUE;
//!     END_CASE;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{Library, TypeName},
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{Case, Expr},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::IntermediateType,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleCaseSelectorType {
            type_environment: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Returns true if a `CASE` may select on a value of this type: `ANY_INT`,
/// a subrange of one, or an enumeration.
fn is_selectable(representation: &IntermediateType) -> bool {
    matches!(
        representation,
        IntermediateType::Int { .. }
            | IntermediateType::UInt { .. }
            | IntermediateType::Subrange { .. }
            | IntermediateType::Enumeration { .. }
    )
}

struct RuleCaseSelectorType<'a> {
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleCaseSelectorType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleCaseSelectorType<'_> {
    /// The resolved type of `selector` when a `CASE` cannot select on it, or
    /// `None` when it can or when the type is not known.
    fn unselectable_type<'e>(&self, selector: &'e Expr) -> Option<&'e TypeName> {
        let representation = self.type_environment.representation_of_expr(selector)?;
        if is_selectable(representation) {
            return None;
        }
        selector.resolved_type.as_ref()
    }
}

impl Visitor<Infallible> for RuleCaseSelectorType<'_> {
    type Value = ();

    fn visit_case(&mut self, node: &Case) -> Result<Self::Value, Infallible> {
        if let Some(actual) = self.unselectable_type(&node.selector) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::CaseSelectorTypeInvalid,
                    Label::span(node.selector.span(), "CASE selector"),
                )
                .with_context("actual", &actual.to_string()),
            );
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use rstest::rstest;

    /// A program whose only `CASE` selects on `sel`, declared as
    /// `declared_type`, with a plain integer label.
    fn program_selecting_on(declared_type: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    sel : {declared_type};
    y : INT;
END_VAR
    CASE sel OF
        1: y := 1;
    END_CASE;
END_PROGRAM"
        )
    }

    fn diagnostics_for(program: &str) -> Vec<Diagnostic> {
        let (library, context) = parse_and_resolve_types_with_context(program);
        apply(&library, &context, &CompilerOptions::default())
            .err()
            .unwrap_or_default()
    }

    #[rstest]
    #[case::sint("SINT")]
    #[case::int("INT")]
    #[case::dint("DINT")]
    #[case::lint("LINT")]
    #[case::usint("USINT")]
    #[case::uint("UINT")]
    #[case::udint("UDINT")]
    #[case::ulint("ULINT")]
    fn apply_when_selector_is_integer_then_ok(#[case] declared_type: &str) {
        assert!(diagnostics_for(&program_selecting_on(declared_type)).is_empty());
    }

    #[rstest]
    #[case::real("REAL")]
    #[case::lreal("LREAL")]
    #[case::bool("BOOL")]
    #[case::byte("BYTE")]
    #[case::word("WORD")]
    #[case::dword("DWORD")]
    #[case::lword("LWORD")]
    #[case::string("STRING")]
    #[case::time("TIME")]
    #[case::date("DATE")]
    fn apply_when_selector_is_not_integer_then_p4052(#[case] declared_type: &str) {
        let diagnostics = diagnostics_for(&program_selecting_on(declared_type));

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, Problem::CaseSelectorTypeInvalid.code());
    }

    rule_ctx_ok!(
        apply_when_selector_is_enumeration_then_ok,
        "
TYPE
    Mode : (Idle, Fill, Drain);
END_TYPE

PROGRAM main
VAR
    mode : Mode;
    y : INT;
END_VAR
    CASE mode OF
        Fill: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_selector_is_named_subrange_then_ok,
        "
TYPE
    Pct : INT (0..100);
END_TYPE

PROGRAM main
VAR
    level : Pct;
    y : INT;
END_VAR
    CASE level OF
        90..100: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// An alias of an integer type resolves to the integer type.
        apply_when_selector_is_alias_of_integer_then_ok,
        "
TYPE
    Counter : DINT := 0;
END_TYPE

PROGRAM main
VAR
    n : Counter;
    y : INT;
END_VAR
    CASE n OF
        1: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// A bare literal resolves to `ANY_INT`, which is not in the type
        /// environment, so the rule leaves it alone.
        apply_when_selector_is_integer_literal_then_ok,
        "
PROGRAM main
VAR
    y : INT;
END_VAR
    CASE 1 OF
        1: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// The selector is an expression, and its type is what is judged.
        apply_when_selector_is_integer_expression_then_ok,
        "
PROGRAM main
VAR
    a : INT;
    b : INT;
    y : INT;
END_VAR
    CASE a + b OF
        1: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_selector_is_alias_of_real_then_p4052,
        "
TYPE
    Level : REAL := 0.0;
END_TYPE

PROGRAM main
VAR
    level : Level;
    y : INT;
END_VAR
    CASE level OF
        1: y := 1;
    END_CASE;
END_PROGRAM",
        Problem::CaseSelectorTypeInvalid
    );

    rule_ctx_err1!(
        apply_when_selector_is_real_expression_then_p4052,
        "
PROGRAM main
VAR
    level : REAL;
    y : INT;
END_VAR
    CASE level * 2.0 OF
        1: y := 1;
    END_CASE;
END_PROGRAM",
        Problem::CaseSelectorTypeInvalid
    );

    #[test]
    fn apply_when_selector_is_real_then_diagnostic_labels_selector_and_names_type() {
        let program = program_selecting_on("REAL");
        let diagnostics = diagnostics_for(&program);

        assert_eq!(diagnostics.len(), 1);
        let start = program.find("sel OF").unwrap();
        assert_eq!(diagnostics[0].primary.location.start, start);
        assert_eq!(diagnostics[0].primary.location.end, start + "sel".len());
        assert!(
            diagnostics[0].described.contains(&"actual=real".to_owned()),
            "{:?}",
            diagnostics[0].described
        );
    }
}
