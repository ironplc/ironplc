//! Semantic rule that the control variable of a `FOR` loop is an integer.
//!
//! IEC 61131-3 requires "the control variable, initial value, and final
//! value" of a `FOR` statement to be "expressions of the same integer type"
//! (see 3.3.2.4). The initial and final values and the step are converted to
//! the control variable's type, so holding the control variable to an
//! integer holds the loop to one. This rule accepts a signed or unsigned
//! integer and a subrange, which narrows an integer type.
//!
//! Every other type is reported. A `REAL` or `LREAL` counter accumulates
//! rounding error with each step, so the number of iterations depends on the
//! values rather than on the bounds. A bit string (`BYTE`, `WORD`, `DWORD`,
//! `LWORD`) is a pattern rather than a magnitude, and a `BOOL`, a duration,
//! a date or an enumeration has no integer step.
//!
//! A control variable whose declared type the analyzer did not resolve is
//! skipped rather than reported.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : INT;
//!     total : INT;
//! END_VAR
//!     FOR i := 1 TO 10 DO
//!         total := total + i;
//!     END_FOR;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     r : REAL;
//!     total : REAL;
//! END_VAR
//!     FOR r := 1.0 TO 3.0 DO     (* P4091: REAL is not an integer *)
//!         total := total + r;
//!     END_FOR;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::Library,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::For,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    symbol_environment::ScopeTracker,
    value_type,
    variable_type::declared_type_id,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleForControlType {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Returns true if a `FOR` loop may count with a variable of this type:
/// `ANY_INT` or a subrange of one.
///
/// `rule_real_literal_integer_position` checks the bounds and step of a loop
/// this rule accepts, and asks the same question, so that exactly one of the
/// two rules reports a loop that counts from a real bound with a non-integer.
pub(crate) fn is_countable(representation: &SemanticType) -> bool {
    matches!(
        representation,
        SemanticType::Int { .. } | SemanticType::UInt { .. } | SemanticType::Subrange { .. }
    )
}

struct RuleForControlType<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look the control variable up.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleForControlType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleForControlType<'_> {
    /// The type of the control variable of `node`, as a diagnostic shows it,
    /// when a `FOR` loop cannot count with it; `None` when it can or when the
    /// type is not known.
    fn uncountable_type(&self, node: &For) -> Option<String> {
        let id = declared_type_id(&node.control, self.context.symbols(), &self.scope.current())?;
        let types = self.context.types();
        if is_countable(&types.get_by_id(id)?.representation) {
            return None;
        }
        Some(value_type::describe(types, id))
    }
}

impl Visitor<Infallible> for RuleForControlType<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_for(&mut self, node: &For) -> Result<Self::Value, Infallible> {
        if let Some(actual) = self.uncountable_type(node) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::ForControlTypeInvalid,
                    Label::span(node.control.span(), "FOR control variable"),
                )
                .with_context("actual", &actual),
            );
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{diagnostic_codes, edition3_options, rule_diagnostics};
    use rstest::rstest;

    /// A program whose only `FOR` loop counts with `x`, declared as
    /// `declared_type`, from `from` to `to`.
    fn program_counting_with(declared_type: &str, from: &str, to: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    x : {declared_type};
    n : INT;
END_VAR
    FOR x := {from} TO {to} DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
        )
    }

    fn codes_for(program: &str) -> Vec<String> {
        diagnostic_codes(&rule_diagnostics(apply, program, &edition3_options()))
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    fn p4091() -> Vec<String> {
        vec![Problem::ForControlTypeInvalid.code().to_string()]
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
    fn apply_when_control_is_integer_then_ok(#[case] declared_type: &str) {
        assert!(codes_for(&program_counting_with(declared_type, "1", "3")).is_empty());
    }

    #[rstest]
    #[case::real("REAL", "1.0", "3.0")]
    #[case::lreal("LREAL", "1.0", "3.0")]
    #[case::bool("BOOL", "FALSE", "TRUE")]
    #[case::byte("BYTE", "1", "3")]
    #[case::word("WORD", "1", "3")]
    #[case::dword("DWORD", "1", "3")]
    #[case::lword("LWORD", "1", "3")]
    #[case::time("TIME", "T#1s", "T#3s")]
    #[case::ltime("LTIME", "LTIME#1s", "LTIME#3s")]
    fn apply_when_control_is_not_integer_then_p4091(
        #[case] declared_type: &str,
        #[case] from: &str,
        #[case] to: &str,
    ) {
        assert_eq!(
            codes_for(&program_counting_with(declared_type, from, to)),
            p4091()
        );
    }

    rule_err!(
        apply_when_control_is_enumeration_then_p4091,
        "
TYPE
    Mode : (Idle, Fill, Drain);
END_TYPE

PROGRAM main
VAR
    mode : Mode;
    n : INT;
END_VAR
    FOR mode := Idle TO Drain DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::ForControlTypeInvalid]
    );

    rule_ok!(
        apply_when_control_is_named_subrange_then_ok,
        "
TYPE
    Slot : INT (1..8);
END_TYPE

PROGRAM main
VAR
    slot : Slot;
    n : INT;
END_VAR
    FOR slot := 1 TO 8 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// An alias of an integer type resolves to the integer type.
        apply_when_control_is_alias_of_integer_then_ok,
        "
TYPE
    Counter : DINT := 0;
END_TYPE

PROGRAM main
VAR
    i : Counter;
    n : INT;
END_VAR
    FOR i := 1 TO 3 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_err!(
        apply_when_control_is_alias_of_real_then_p4091,
        "
TYPE
    Level : REAL := 0.0;
END_TYPE

PROGRAM main
VAR
    level : Level;
    n : INT;
END_VAR
    FOR level := 1.0 TO 3.0 DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::ForControlTypeInvalid]
    );

    rule_err!(
        /// The control variable is looked up from the scope of the body
        /// that counts with it: a function's own variable here.
        apply_when_function_counts_with_real_then_p4091,
        "
FUNCTION Sum : INT
VAR
    r : REAL;
END_VAR
    FOR r := 1.0 TO 3.0 DO
        Sum := Sum + 1;
    END_FOR;
END_FUNCTION",
        [Problem::ForControlTypeInvalid]
    );

    rule_ok!(
        /// A method counts with a field of its block, which is an integer.
        apply_when_method_counts_with_integer_field_then_ok,
        "
FUNCTION_BLOCK Block
VAR
    i : INT;
    n : INT;
END_VAR
METHOD Count
    FOR i := 1 TO 3 DO
        n := n + 1;
    END_FOR;
END_METHOD
END_FUNCTION_BLOCK",
        edition3_options()
    );

    rule_err!(
        /// A method's own variable shadows nothing here, and is a `REAL`.
        apply_when_method_counts_with_real_then_p4091,
        "
FUNCTION_BLOCK Block
VAR
    n : INT;
END_VAR
METHOD Count
VAR
    r : REAL;
END_VAR
    FOR r := 1.0 TO 3.0 DO
        n := n + 1;
    END_FOR;
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::ForControlTypeInvalid],
        edition3_options()
    );

    rule_err!(
        /// Each loop is judged on its own: the inner one counts with a
        /// `REAL`, the outer with an integer.
        apply_when_nested_loop_counts_with_real_then_p4091_once,
        "
PROGRAM main
VAR
    i : INT;
    r : REAL;
    n : INT;
END_VAR
    FOR i := 1 TO 3 DO
        FOR r := 1.0 TO 3.0 DO
            n := n + 1;
        END_FOR;
    END_FOR;
END_PROGRAM",
        [Problem::ForControlTypeInvalid]
    );

    #[test]
    fn apply_when_control_is_real_then_p4091_at_control_variable() {
        let program = "
PROGRAM main
VAR
    level : REAL;
    n : INT;
END_VAR
    FOR level := 1.0 TO 3.0 DO
        n := n + 1;
    END_FOR;
END_PROGRAM";
        let diagnostics = rule_diagnostics(apply, program, &edition3_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::ForControlTypeInvalid.code()]
        );
        let start = program.find("FOR level").unwrap() + "FOR ".len();
        let location = &diagnostics[0].primary.location;
        assert_eq!(
            (location.start, location.end),
            (start, start + "level".len())
        );
    }
}
