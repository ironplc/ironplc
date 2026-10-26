//! Semantic rule that the inputs a function of several inputs of one type
//! chooses between -- those of `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX` and
//! `ATAN2` -- have a type every one of them widens to.
//!
//! Such a function computes at the type every input of the function's type
//! widens to, and its result has that type (see
//! `specs/design/implicit-conversions.md`, "Standard functions"):
//! `MAX(i, l)` on an `INT` and an `LINT` compares at `LINT`. A set of inputs
//! none of whose types holds every other one -- a `DINT` and a `UDINT`, or a
//! `DINT` and a `REAL` -- has no type to compute at. The resolver gives such
//! a call the type of its first concrete input so that later passes see a
//! type, and the other inputs are converted to it: `MAX(d, u)` with
//! `u = 4000000000` read the `UDINT` as a negative `DINT` and chose `d`
//! (#2127). This rule reports the call as P4049, the code the operator rule
//! reports for `d + u`, naming the function as the operator and the type of
//! each input by its parameter.
//!
//! The inputs are judged by the relation the resolver types the call by
//! ([`common_operand_of`]), so what this rule accepts is what the resolver
//! types and a backend computes at. Only the inputs of the function's type
//! take part: `SEL`'s `G` and `MUX`'s `K` select between the others and have
//! types of their own. An untyped literal takes the other inputs' type when
//! its category allows, so `MAX(d, 1)` passes and `MAX(d, 1.5)` is reported,
//! as `d + 1.5` is; a real literal beside an integer that widens to `REAL`
//! is a `REAL`, so `MIN(i, 1.5)` on an `INT` passes, as `i + 1.5` does.
//! `EXPT` is typed by its base, with the exponent converted
//! to it, so it has nothing to check, and the function forms of `AND`, `OR`
//! and `XOR` are the operator rule's.
//!
//! An input the analyzer left without a type, or whose type its parameter
//! does not accept, ends the check of its call: the rule that failed to type
//! it, or the argument type check (P4026), has already said why.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : INT;
//!     l : LINT;
//!     d : DINT;
//!     u : UDINT;
//! END_VAR
//!     l := MAX(i, l);          (* compares at LINT *)
//!     l := LIMIT(d, u, l);     (* a DINT and a UDINT both widen to LINT *)
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     d : DINT;
//!     u : UDINT;
//!     r : REAL;
//!     g : BOOL;
//!     l : LINT;
//! END_VAR
//!     l := MAX(d, u);          (* P4049: neither type holds the other *)
//!     l := SEL(g, d, u);       (* P4049: G takes no part *)
//!     r := MAX(d, r);          (* P4049: a DINT does not widen to REAL *)
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::{Library, TypeName},
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{Expr, ExprKind, Function},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    builtin_function::{BuiltinFunction, OneTypeResult},
    function_environment::FunctionEnvironment,
    intermediates::common_operand::common_operand_of,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_compat::are_types_compatible,
    type_environment::TypeEnvironment,
    value_type::{operand_type_name, parameter_type_name},
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleFunctionInputsCommonType {
            types: context.types(),
            functions: context.functions(),
            options,
            diagnostics: vec![],
        },
        lib,
    )
}

struct RuleFunctionInputsCommonType<'a> {
    types: &'a TypeEnvironment,
    functions: &'a FunctionEnvironment,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleFunctionInputsCommonType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleFunctionInputsCommonType<'_> {
    /// Reports P4049 when the inputs of `call` that have the function's type
    /// have no type every one of them widens to.
    fn check_call(&mut self, call: &Function) {
        let Some(signature) = self.functions.get(&call.name) else {
            return;
        };
        let Some(builtin) = &signature.builtin_function else {
            return;
        };
        // The function forms of the operators are the operator rule's.
        if let BuiltinFunction::Operator(_) = builtin {
            return;
        }
        let Some(shape) = builtin.inputs_of_one_type() else {
            return;
        };
        if shape.result != OneTypeResult::Common {
            return;
        }
        // Each input of the function's type, as its parameter's name and the
        // name the relation knows its type by. An input without a type, or
        // of a type its parameter does not accept, is another rule's.
        let mut inputs = Vec::new();
        for (parameter, input) in signature
            .inputs_of(&call.param_assignment)
            .skip(shape.first)
        {
            let Some(name) = input
                .expr_type
                .as_ref()
                .and_then(|expr_type| operand_type_name(self.types, expr_type))
            else {
                return;
            };
            let Some(param_type) = parameter_type_name(self.types, &parameter.param_type) else {
                return;
            };
            if !are_types_compatible(&param_type, &name, self.options) {
                return;
            }
            inputs.push((parameter.name, name));
        }
        // Fewer than two inputs have nothing to agree on; a call that
        // supplies too few is reported by the binding of its arguments.
        if inputs.len() < 2 {
            return;
        }
        let names: Vec<Option<&TypeName>> = inputs.iter().map(|(_, name)| Some(name)).collect();
        if common_operand_of(&names, self.options).is_some() {
            return;
        }
        let mut diagnostic = Diagnostic::problem(
            Problem::OperatorOperandTypeMismatch,
            Label::span(call.name.span(), "Function call"),
        )
        .with_context("operator", call.name.original());
        for (parameter, name) in &inputs {
            diagnostic =
                diagnostic.with_context(parameter.original(), &name.to_string().to_uppercase());
        }
        self.diagnostics.push(diagnostic);
    }
}

impl Visitor<Infallible> for RuleFunctionInputsCommonType<'_> {
    type Value = ();

    fn visit_expr(&mut self, node: &Expr) -> Result<Self::Value, Infallible> {
        if let ExprKind::Function(call) = &node.kind {
            self.check_call(call);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{codes, diagnostic_codes, rule_codes, rule_diagnostics};
    use ironplc_parser::options::Dialect;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// A program with one variable of each type the tests use, whose body
    /// assigns `value` to the `LREAL` `x`.
    fn program_with(value: &str) -> String {
        format!(
            "
TYPE
    Small : DINT (0..100);
END_TYPE

PROGRAM main
VAR
    x : LREAL;
    i : INT;
    d : DINT;
    u : UDINT;
    l : LINT;
    r : REAL;
    lr : LREAL;
    g : BOOL;
    k : INT;
    s : STRING;
    m : Small;
    w : WORD;
    t : TIME;
END_VAR
    x := {value};
END_PROGRAM"
        )
    }

    fn codes_for(value: &str, options: &CompilerOptions) -> Vec<String> {
        rule_codes(apply, &program_with(value), options)
    }

    fn p4049() -> String {
        Problem::OperatorOperandTypeMismatch.code().to_string()
    }

    #[spec_test(REQ_IC_analyzer_125)]
    #[rstest]
    #[case::max_of_signed_and_unsigned("MAX(d, u)")]
    #[case::min_with_the_unsigned_first("MIN(u, d)")]
    #[case::sel_after_its_selector("SEL(g, d, u)")]
    #[case::mux_after_its_selector("MUX(k, d, u, r)")]
    #[case::integer_and_real("MAX(d, r)")]
    #[case::integer_and_real_literal("MAX(d, 1.5)")]
    #[case::subrange_at_its_base_type("MAX(m, u)")]
    #[case::named_inputs("MAX(IN2 := u, IN1 := d)")]
    #[case::reported_once_in_an_outer_call("MAX(MIN(d, u), l)")]
    #[case::sel_of_a_duration_and_an_integer("SEL(g, t, d)")]
    #[case::mux_of_integers_and_a_string("MUX(k, d, d, d, s)")]
    fn apply_when_no_input_type_holds_every_other_then_p4049(#[case] value: &str) {
        assert_eq!(codes_for(value, &CompilerOptions::default()), vec![p4049()]);
    }

    #[test]
    fn apply_when_no_input_type_holds_every_other_in_every_dialect_then_p4049() {
        for dialect in Dialect::ALL {
            let options = CompilerOptions::from_dialect(*dialect);
            assert_eq!(
                codes_for("MAX(d, u)", &options),
                vec![p4049()],
                "{dialect:?}"
            );
        }
    }

    #[spec_test(REQ_IC_analyzer_125)]
    #[rstest]
    #[case::widest_last("MAX(i, l)")]
    #[case::widest_first("MIN(l, i)")]
    #[case::same_type("MAX(d, d)")]
    #[case::widest_after_a_pair_that_does_not_widen("LIMIT(d, u, l)")]
    #[case::mux_inputs_widen_to_the_last("MUX(k, d, u, l)")]
    #[case::integer_literal_takes_the_other_type("MAX(d, 1)")]
    #[case::real_literal_with_a_real("MAX(r, 1.5)")]
    #[case::real_literal_with_a_narrow_integer("MIN(i, 1.5)")]
    #[case::real_literals_around_a_narrow_integer("LIMIT(0.5, i, 2.5)")]
    #[case::literals_only("MAX(1, 2.5)")]
    #[case::real_and_long_real("ATAN2(r, lr)")]
    #[case::sel_of_strings("SEL(g, s, s)")]
    #[case::sel_of_bools("SEL(g, g, g)")]
    #[case::mux_of_durations("MUX(k, t, t)")]
    fn apply_when_every_input_widens_to_one_then_ok(#[case] value: &str) {
        assert!(codes_for(value, &CompilerOptions::default()).is_empty());
    }

    #[spec_test(REQ_IC_analyzer_126)]
    #[rstest]
    #[case::sel_selector_takes_no_part("SEL(g, d, d)")]
    #[case::mux_selector_takes_no_part("MUX(l, d, d)")]
    #[case::input_its_parameter_rejects_is_p4026("MAX(d, s)")]
    #[case::integer_input_of_atan2_is_p4026("ATAN2(d, r)")]
    #[case::expt_is_typed_by_its_base("EXPT(d, u)")]
    #[case::bitwise_function_form_is_the_operator_rule("AND(g, w)")]
    #[case::too_few_inputs_is_the_binding("MAX(d)")]
    #[case::no_inputs_is_the_binding("SEL(g)")]
    fn apply_when_input_takes_no_part_then_ok(#[case] value: &str) {
        assert!(codes_for(value, &CompilerOptions::default()).is_empty());
    }

    rule_err_at!(
        apply_when_reported_then_labelled_at_the_function_name,
        &program_with("MAX(d, u)"),
        Problem::OperatorOperandTypeMismatch,
        "MAX"
    );

    #[spec_test(REQ_IC_analyzer_125)]
    #[test]
    fn apply_when_reported_then_names_the_function_and_each_input_type() {
        let diagnostics = rule_diagnostics(
            apply,
            &program_with("MUX(k, d, u, r)"),
            &CompilerOptions::default(),
        );
        assert_eq!(
            diagnostic_codes(&diagnostics),
            codes(&[Problem::OperatorOperandTypeMismatch])
        );
        assert_eq!(
            diagnostics[0].described,
            ["operator=MUX", "IN0=DINT", "IN1=UDINT", "IN2=REAL"]
        );
    }

    rule_ok!(
        apply_when_user_function_then_ok,
        "
FUNCTION PICK : LINT
VAR_INPUT
    d : DINT;
    u : UDINT;
END_VAR
    PICK := 0;
END_FUNCTION

PROGRAM main
VAR
    d : DINT;
    u : UDINT;
    l : LINT;
END_VAR
    l := PICK(d, u);
END_PROGRAM"
    );

    // The example on the P4049 documentation page.
    rule_err!(
        apply_when_documented_example_then_p4049,
        "
PROGRAM main
VAR
    lowest : DINT := -1;
    count : UDINT := 4000000000;
    largest : LINT;
END_VAR
    largest := MAX(lowest, count);
END_PROGRAM",
        [Problem::OperatorOperandTypeMismatch]
    );

    rule_ok!(
        apply_when_documented_fix_then_ok,
        "
PROGRAM main
VAR
    lowest : DINT := -1;
    count : UDINT := 4000000000;
    largest : LINT;
END_VAR
    largest := MAX(DINT_TO_LINT(lowest), UDINT_TO_LINT(count));
END_PROGRAM"
    );
}
