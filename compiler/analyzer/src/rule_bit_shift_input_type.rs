//! Semantic rule that the value a shift or rotate function shifts is not a
//! `BOOL`.
//!
//! IEC 61131-3 declares `SHL`, `SHR`, `ROL` and `ROR` on `ANY_BIT`, which
//! includes `BOOL`, but a shift of one bit has no meaning a program relies
//! on: shifting it by one either keeps it or clears it depending on how wide
//! the implementation computes, and a rotation leaves it as it is. IronPLC
//! accepts `BYTE`, `WORD`, `DWORD` and `LWORD` for the shifted input, as its
//! reference pages state, and reports a `BOOL` in every dialect, so no flag
//! relaxes the rule.
//!
//! The input is judged by the type the analyzer resolved for it, so a `BOOL`
//! variable, an alias of `BOOL`, a comparison, a bit access (`w.3`), a `BOOL`
//! literal and a function result of type `BOOL` are each reported, whether
//! the input is passed by position or as `IN :=`. An input the analyzer left
//! without a type is skipped: the rule that failed to type it has already
//! said why. The count `N` is an integer, which the argument type check
//! (P4026) enforces.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     flags : BYTE;
//! END_VAR
//!     flags := SHL(flags, 1);
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     ready : BOOL;
//! END_VAR
//!     ready := SHL(ready, 1);         (* P4087: ready is a BOOL *)
//!     ready := ROR(IN := ready, N := 1); (* P4087 *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::Library,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::{Expr, ExprType, Function},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    builtin_function::BuiltinFunction,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleBitShiftInputType {
            context,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleBitShiftInputType<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleBitShiftInputType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleBitShiftInputType<'_> {
    /// Whether the analyzer resolved `value` to `BOOL` or an alias of it.
    fn is_bool(&self, value: &Expr) -> bool {
        let Some(ExprType::Concrete(id) | ExprType::Inferred(id)) = &value.expr_type else {
            return false;
        };
        self.context
            .types()
            .get_by_id(*id)
            .is_some_and(|attributes| attributes.representation == SemanticType::Bool)
    }
}

impl Visitor<Infallible> for RuleBitShiftInputType<'_> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        let shifted = self
            .context
            .functions
            .get(&node.name)
            .filter(|signature| {
                matches!(
                    signature.builtin_function,
                    Some(BuiltinFunction::BitShift(_))
                )
            })
            .and_then(|signature| {
                signature
                    .inputs_of(&node.param_assignment)
                    .find(|(param, _)| param.name == Id::from("IN"))
                    .map(|(_, value)| value)
            });
        if let Some(value) = shifted.filter(|value| self.is_bool(value)) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::BitShiftInputBool,
                    Label::span(value.span(), "Shifted value"),
                )
                .with_context("function", &node.name.original().to_string()),
            );
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::rule_codes;
    use ironplc_parser::options::Dialect;
    use rstest::rstest;

    /// A program with variables `b : BOOL`, `c : BOOL`, `w : WORD` and
    /// `i : DINT`, whose body is `statement`.
    fn program_with(statement: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    b : BOOL;
    c : BOOL;
    w : WORD;
    i : DINT;
END_VAR
    {statement}
END_PROGRAM"
        )
    }

    fn codes_for(program: &str, options: &CompilerOptions) -> Vec<String> {
        rule_codes(apply, program, options)
    }

    fn p4087() -> String {
        Problem::BitShiftInputBool.code().to_string()
    }

    #[rstest]
    #[case::shl("b := SHL(b, 1);")]
    #[case::shr("b := SHR(b, 1);")]
    #[case::rol("b := ROL(b, 1);")]
    #[case::ror("b := ROR(b, 1);")]
    #[case::named("b := SHL(IN := b, N := 1);")]
    #[case::named_count_first("b := SHL(N := 1, IN := b);")]
    #[case::literal("b := SHL(TRUE, 1);")]
    #[case::comparison("b := SHL(i > 0, 1);")]
    #[case::bit_access("b := SHL(w.3, 1);")]
    #[case::boolean_operator("b := SHL(b AND c, 1);")]
    #[case::count_zero("b := SHL(b, 0);")]
    #[case::in_condition("IF ROL(b, 1) THEN i := 1; END_IF;")]
    #[case::inside_another_shift("w := SHL(w, BOOL_TO_INT(SHR(b, 1)));")]
    fn apply_when_shifted_value_is_bool_then_p4087(#[case] statement: &str) {
        assert_eq!(
            codes_for(&program_with(statement), &CompilerOptions::default()),
            vec![p4087()]
        );
    }

    #[test]
    fn apply_when_shifted_value_is_bool_in_every_dialect_then_p4087() {
        for dialect in Dialect::ALL {
            let options = CompilerOptions::from_dialect(*dialect);
            assert_eq!(
                codes_for(&program_with("b := SHL(b, 1);"), &options),
                vec![p4087()],
                "{dialect:?}"
            );
        }
    }

    #[rstest]
    #[case::word("w := SHL(w, 1);")]
    #[case::word_rotated("w := ROR(IN := w, N := 3);")]
    #[case::bool_count("w := SHL(w, BOOL_TO_INT(b));")]
    #[case::converted_bool("w := SHL(BOOL_TO_WORD(b), 1);")]
    fn apply_when_shifted_value_is_bit_string_then_ok(#[case] statement: &str) {
        assert!(codes_for(&program_with(statement), &CompilerOptions::default()).is_empty());
    }

    rule_err!(
        apply_when_shifted_value_is_alias_of_bool_then_p4087,
        "
TYPE
    Flag : BOOL := FALSE;
END_TYPE

PROGRAM main
VAR
    f : Flag;
END_VAR
    f := SHL(f, 1);
END_PROGRAM",
        [Problem::BitShiftInputBool]
    );

    rule_err!(
        apply_when_shifted_value_is_bool_function_result_then_p4087,
        "
FUNCTION is_high : BOOL
VAR_INPUT
    level : INT;
END_VAR
    is_high := level > 90;
END_FUNCTION

PROGRAM main
VAR
    level : INT;
    b : BOOL;
END_VAR
    b := ROL(is_high(level), 1);
END_PROGRAM",
        [Problem::BitShiftInputBool]
    );

    rule_ok!(
        /// A shifted value the analyzer did not type is left to the rule that
        /// failed to type it.
        apply_when_shifted_value_is_untyped_then_ok,
        "
PROGRAM main
VAR
    b : BOOL;
END_VAR
    b := SHL(undeclared, 1);
END_PROGRAM"
    );

    rule_ok!(
        /// A user-defined function is not a standard shift, whatever its
        /// input is called.
        apply_when_user_function_takes_bool_in_then_ok,
        "
FUNCTION SHIFT_FLAG : BOOL
VAR_INPUT
    IN : BOOL;
    N : INT;
END_VAR
    SHIFT_FLAG := IN;
END_FUNCTION

PROGRAM main
VAR
    b : BOOL;
END_VAR
    b := SHIFT_FLAG(IN := b, N := 1);
END_PROGRAM"
    );

    rule_err_at!(
        apply_when_shifted_value_is_comparison_then_p4087_at_value,
        &program_with("b := SHL(i > 0, 1);"),
        Problem::BitShiftInputBool,
        "i > 0"
    );
}
