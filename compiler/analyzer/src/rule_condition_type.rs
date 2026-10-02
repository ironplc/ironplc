//! Semantic rule that the condition of an `IF`, `ELSIF`, `WHILE` or
//! `REPEAT ... UNTIL` statement is a `BOOL`.
//!
//! IEC 61131-3 defines each of these statements over a Boolean expression
//! (see 3.3.2.3 and 3.3.2.4). A condition of another type is a C idiom --
//! "non-zero is true" -- that the standard does not have, and a program that
//! relies on it compiled before this rule existed but should not have.
//!
//! The condition is judged by the relation an assignment to a `BOOL`
//! variable uses ([`value_type::check`]), so the two cannot disagree: a
//! comparison, a `BOOL` variable, an alias of `BOOL`, a bit access (`w.3`),
//! and a function result or function block output of type `BOOL` are
//! accepted; an integer, a bit string, a real, a string, a time or an
//! enumeration is reported, and so is an untyped integer literal, as
//! `b := 1` is. No dialect is known to accept an integer condition, so no
//! flag relaxes the rule.
//!
//! A condition the analyzer left without a type is skipped rather than
//! reported: there is nothing to compare, and the rule that failed to type
//! it has already said why.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     level : INT;
//!     alarm : BOOL;
//!     flags : WORD;
//! END_VAR
//!     IF level > 90 THEN alarm := TRUE; END_IF;
//!     WHILE alarm DO alarm := FALSE; END_WHILE;
//!     REPEAT level := level - 1; UNTIL flags.3 END_REPEAT;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     level : INT;
//!     alarm : BOOL;
//! END_VAR
//!     IF level THEN          (* P4072: INT is not BOOL *)
//!         alarm := TRUE;
//!     END_IF;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{Library, TypeName},
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{ElseIf, Expr, If, Repeat, While},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
    value_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleConditionType {
            type_environment: context.types(),
            options,
            bool_type: TypeName::from("BOOL"),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleConditionType<'a> {
    type_environment: &'a TypeEnvironment,
    options: &'a CompilerOptions,
    bool_type: TypeName,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleConditionType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleConditionType<'_> {
    /// Reports `condition` when its value cannot be used where a `BOOL` is
    /// required. `statement` names the keyword the condition follows.
    fn check_condition(&mut self, condition: &Expr, statement: &str) {
        let Err(mismatch) = value_type::check(
            self.type_environment,
            &self.bool_type,
            condition,
            self.options,
        ) else {
            return;
        };
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ConditionTypeInvalid,
                Label::span(condition.span(), format!("{statement} condition")),
            )
            .with_context("actual", &mismatch.actual),
        );
    }
}

impl Visitor<Infallible> for RuleConditionType<'_> {
    type Value = ();

    fn visit_if(&mut self, node: &If) -> Result<Self::Value, Infallible> {
        self.check_condition(&node.expr, "IF");
        node.recurse_visit(self)
    }

    fn visit_else_if(&mut self, node: &ElseIf) -> Result<Self::Value, Infallible> {
        self.check_condition(&node.expr, "ELSIF");
        node.recurse_visit(self)
    }

    fn visit_while(&mut self, node: &While) -> Result<Self::Value, Infallible> {
        self.check_condition(&node.condition, "WHILE");
        node.recurse_visit(self)
    }

    fn visit_repeat(&mut self, node: &Repeat) -> Result<Self::Value, Infallible> {
        self.check_condition(&node.until, "UNTIL");
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use rstest::rstest;

    /// A program with variables `c` of `declared_type`, `b : BOOL`,
    /// `w : WORD` and `y : INT`, whose body is `statement`.
    fn program_with(declared_type: &str, statement: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    c : {declared_type};
    b : BOOL;
    w : WORD;
    y : INT;
END_VAR
    {statement}
END_PROGRAM"
        )
    }

    fn diagnostics_for(program: &str) -> Vec<Diagnostic> {
        let (library, context) = parse_and_resolve_types_with_context(program);
        apply(&library, &context, &CompilerOptions::default())
            .err()
            .unwrap_or_default()
    }

    fn codes_for(program: &str) -> Vec<String> {
        diagnostics_for(program)
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    fn p4072() -> String {
        Problem::ConditionTypeInvalid.code().to_string()
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
    #[case::real("REAL")]
    #[case::lreal("LREAL")]
    #[case::byte("BYTE")]
    #[case::word("WORD")]
    #[case::dword("DWORD")]
    #[case::lword("LWORD")]
    #[case::string("STRING")]
    #[case::wstring("WSTRING")]
    #[case::time("TIME")]
    #[case::ltime("LTIME")]
    #[case::date("DATE")]
    #[case::time_of_day("TIME_OF_DAY")]
    #[case::date_and_time("DATE_AND_TIME")]
    fn apply_when_if_condition_is_not_bool_then_p4072(#[case] declared_type: &str) {
        let program = program_with(declared_type, "IF c THEN y := 1; END_IF;");

        assert_eq!(codes_for(&program), vec![p4072()]);
    }

    #[rstest]
    #[case::if_("IF c THEN y := 1; END_IF;")]
    #[case::elsif("IF b THEN y := 1; ELSIF c THEN y := 2; END_IF;")]
    #[case::while_("WHILE c DO y := 1; END_WHILE;")]
    #[case::repeat("REPEAT y := 1; UNTIL c END_REPEAT;")]
    fn apply_when_condition_is_dint_then_p4072(#[case] statement: &str) {
        assert_eq!(codes_for(&program_with("DINT", statement)), vec![p4072()]);
    }

    #[rstest]
    #[case::if_("IF c THEN y := 1; END_IF;")]
    #[case::elsif("IF b THEN y := 1; ELSIF c THEN y := 2; END_IF;")]
    #[case::while_("WHILE c DO y := 1; END_WHILE;")]
    #[case::repeat("REPEAT y := 1; UNTIL c END_REPEAT;")]
    fn apply_when_condition_is_bool_then_ok(#[case] statement: &str) {
        assert!(diagnostics_for(&program_with("BOOL", statement)).is_empty());
    }

    #[rstest]
    #[case::comparison("IF y > 1 THEN y := 1; END_IF;")]
    #[case::bool_literal("IF TRUE THEN y := 1; END_IF;")]
    #[case::bool_operators("IF b AND NOT b OR b XOR b THEN y := 1; END_IF;")]
    #[case::parenthesized("IF (y = 1) THEN y := 1; END_IF;")]
    #[case::bit_access("IF w.3 THEN y := 1; END_IF;")]
    #[case::bool_conversion("IF INT_TO_BOOL(y) THEN y := 1; END_IF;")]
    #[case::comparison_in_loop("WHILE y < 10 DO y := y + 1; END_WHILE;")]
    fn apply_when_condition_is_bool_expression_then_ok(#[case] statement: &str) {
        assert!(diagnostics_for(&program_with("DINT", statement)).is_empty());
    }

    #[rstest]
    #[case::int_literal("IF 1 THEN y := 1; END_IF;")]
    #[case::real_literal("IF 1.0 THEN y := 1; END_IF;")]
    #[case::integer_and("IF c AND c THEN y := 1; END_IF;")]
    #[case::word_and("IF w AND WORD#16#0008 THEN y := 1; END_IF;")]
    #[case::arithmetic("WHILE y - 1 DO y := y - 1; END_WHILE;")]
    fn apply_when_condition_is_non_bool_expression_then_p4072(#[case] statement: &str) {
        assert_eq!(codes_for(&program_with("DINT", statement)), vec![p4072()]);
    }

    #[test]
    fn apply_when_each_nested_condition_is_not_bool_then_p4072_for_each() {
        let program = program_with(
            "DINT",
            "WHILE c DO IF c THEN y := 1; ELSIF c THEN y := 2; END_IF; END_WHILE;",
        );

        assert_eq!(codes_for(&program), vec![p4072(), p4072(), p4072()]);
    }

    rule_ctx_ok!(
        /// An alias of `BOOL` resolves to `BOOL`.
        apply_when_condition_is_alias_of_bool_then_ok,
        "
TYPE
    Flag : BOOL := FALSE;
END_TYPE

PROGRAM main
VAR
    f : Flag;
    y : INT;
END_VAR
    IF f THEN y := 1; END_IF;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_condition_is_bool_function_result_then_ok,
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
    y : INT;
END_VAR
    IF is_high(level) THEN y := 1; END_IF;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_condition_is_function_block_bool_output_then_ok,
        "
PROGRAM main
VAR
    delay : TON;
    y : INT;
END_VAR
    delay(IN := TRUE, PT := T#1s);
    IF delay.Q THEN y := 1; END_IF;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// A condition the analyzer did not type is left to the rule that
        /// failed to type it.
        apply_when_condition_is_untyped_then_ok,
        "
PROGRAM main
VAR
    y : INT;
END_VAR
    IF undeclared THEN y := 1; END_IF;
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_condition_is_alias_of_integer_then_p4072,
        "
TYPE
    Counter : DINT := 0;
END_TYPE

PROGRAM main
VAR
    n : Counter;
    y : INT;
END_VAR
    IF n THEN y := 1; END_IF;
END_PROGRAM",
        Problem::ConditionTypeInvalid
    );

    rule_ctx_err1!(
        apply_when_condition_is_enumeration_then_p4072,
        "
TYPE
    Mode : (Idle, Run);
END_TYPE

PROGRAM main
VAR
    mode : Mode;
    y : INT;
END_VAR
    IF mode THEN y := 1; END_IF;
END_PROGRAM",
        Problem::ConditionTypeInvalid
    );

    rule_ctx_err1!(
        apply_when_condition_is_integer_function_result_then_p4072,
        "
FUNCTION count : DINT
VAR_INPUT
    level : INT;
END_VAR
    count := level;
END_FUNCTION

PROGRAM main
VAR
    level : INT;
    y : INT;
END_VAR
    IF count(level) THEN y := 1; END_IF;
END_PROGRAM",
        Problem::ConditionTypeInvalid
    );

    #[test]
    fn apply_when_condition_is_dint_then_diagnostic_labels_condition_and_names_type() {
        let program = program_with("DINT", "WHILE c + 1 DO y := 1; END_WHILE;");
        let diagnostics = diagnostics_for(&program);

        assert_eq!(diagnostics.len(), 1);
        let start = program.find("c + 1").unwrap();
        assert_eq!(diagnostics[0].primary.location.start, start);
        assert_eq!(diagnostics[0].primary.location.end, start + "c + 1".len());
        assert!(
            diagnostics[0].described.contains(&"actual=dint".to_owned()),
            "{:?}",
            diagnostics[0].described
        );
    }
}
