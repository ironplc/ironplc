//! Semantic rule that validates function calls reference declared functions
//! and have the correct number of arguments.
//!
//! A positional call supplies every input and `VAR_IN_OUT`. A formal call of
//! a user-defined function may omit a `VAR_INPUT`, which starts from its
//! initial value, but not a `VAR_IN_OUT`. A call of a standard function
//! supplies every input it declares (see `specs/design/call-binding.md`).
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := INT_TO_REAL(value);
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails (Function Not Declared)
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := NONEXISTENT_FUNC(value);
//! END_FUNCTION_BLOCK
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    builtin_function::{BuiltinFunction, StringFunction},
    call_binding::bind,
    callee_resolution::{CallParameters, ParameterSection},
    function_environment::FunctionSignature,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleFunctionCallDeclared {
            context,
            diagnostics: vec![],
        },
        lib,
    )
}

struct RuleFunctionCallDeclared<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl RuleFunctionCallDeclared<'_> {
    /// Reports each `VAR_IN_OUT` the formal call `node` of the function
    /// `signature` does not supply (P4018): it names the caller's variable,
    /// so it has no initial value to take. Nothing is reported for a call
    /// whose arguments do not bind, as the binding pass reported why.
    fn check_omitted_in_outs(&mut self, node: &Function, signature: &FunctionSignature) {
        let inputs = node
            .param_assignment
            .iter()
            .filter(|arg| arg.input_expr().is_some())
            .count();
        let parameters = CallParameters::of_function(signature, inputs);
        let Ok(binding) = bind(&parameters, &node.param_assignment) else {
            return;
        };
        let missing = binding
            .omitted(&parameters)
            .into_iter()
            .filter(|parameter| parameter.section == ParameterSection::InOut);
        for parameter in missing {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionCallWrongArgCount,
                    Label::span(node.name.span(), "Function call"),
                )
                .with_context("function", &node.name.original().to_string())
                .with_context_id("missing in-out", &parameter.name),
            );
        }
    }
}

impl DiagnosticVisitor for RuleFunctionCallDeclared<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleFunctionCallDeclared<'_> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        // Look up the function in the function environment
        let func_sig = self.context.functions.get(&node.name);

        match func_sig {
            None => {
                // Function is not declared
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::FunctionCallUndeclared,
                        Label::span(node.name.span(), "Function call"),
                    )
                    .with_context("function", &node.name.original().to_string()),
                );
            }
            Some(signature) => {
                // Function exists, check argument count
                // Count positional input arguments in the call
                let call_input_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)))
                    .count();

                // Count named input arguments
                let call_named_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::NamedInput(_)))
                    .count();

                // A call is either formal or non-formal. With both, the
                // count below would be checked against a misreading of the
                // arguments, so the mix is the one diagnostic, as for a
                // function block invocation.
                if call_input_count > 0 && call_named_count > 0 {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallMixedArgTypes,
                            Label::span(node.name.span(), "Function call"),
                        )
                        .with_context("function", &node.name.original().to_string()),
                    );
                    return node.recurse_visit(self);
                }

                // A formal call of a user-defined function may omit a
                // `VAR_INPUT`, which then takes its initial value (IEC
                // 61131-3 ed.3 6.6.1.7.1), so only an omitted `VAR_IN_OUT`
                // is a missing argument.
                if call_input_count == 0 && !signature.is_stdlib() {
                    self.check_omitted_in_outs(node, signature);
                    return node.recurse_visit(self);
                }

                // Total input arguments provided
                let total_inputs = call_input_count + call_named_count;

                // Expected input parameter count
                let expected_inputs = signature.input_parameter_count();

                let args_valid = if signature.is_extensible {
                    let above_min = total_inputs >= expected_inputs;
                    let below_max = signature.max_inputs.is_none_or(|max| total_inputs <= max);
                    above_min && below_max
                } else {
                    total_inputs == expected_inputs
                };

                if !args_valid {
                    let mut diagnostic = Diagnostic::problem(
                        Problem::FunctionCallWrongArgCount,
                        Label::span(node.name.span(), "Function call"),
                    )
                    .with_context("function", &node.name.original().to_string())
                    .with_context("expected", &expected_inputs.to_string())
                    .with_context("actual", &total_inputs.to_string());
                    if is_two_input_concat(signature) && total_inputs > expected_inputs {
                        diagnostic = diagnostic.with_help(TWO_INPUT_CONCAT_HELP);
                    }
                    self.diagnostics.push(diagnostic);
                }
            }
        }

        // Continue visiting children (arguments may contain nested function calls)
        node.recurse_visit(self)
    }
}

/// The help for a call of the two-input `CONCAT` with more inputs: how to
/// write the call in this dialect, and the dialects that take it as written
/// (REQ-CB-analyzer-056).
const TWO_INPUT_CONCAT_HELP: &str = "This dialect's CONCAT takes two inputs, as CODESYS \
and TwinCAT declare it. Nest the calls, as in CONCAT(CONCAT(a, b), c). IEC 61131-3 and \
RuSTy declare CONCAT extensible (--allow-extensible-concat).";

/// Whether `signature` is the standard `CONCAT` narrowed to two inputs, as
/// the analyzer registers it when `allow_extensible_concat` is off.
fn is_two_input_concat(signature: &FunctionSignature) -> bool {
    !signature.is_extensible
        && matches!(
            signature.builtin_function,
            Some(BuiltinFunction::String(StringFunction::Concat))
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{codes, diagnostic_codes, rule_diagnostics};
    use rstest::rstest;
    use spec_test_macro::spec_test;

    rule_ok!(
        apply_when_stdlib_function_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_user_function_called_then_ok,
        "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1, 2);
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_function_not_declared_then_error,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := NONEXISTENT_FUNC(1);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallUndeclared]
    );

    rule_err_at!(
        apply_when_function_calls_undeclared_function_then_error,
        "
FUNCTION MY_FUNC : INT
VAR_INPUT
    x : INT;
END_VAR
    MY_FUNC := UNDEFINED_HELPER(x);
END_FUNCTION",
        Problem::FunctionCallUndeclared,
        "UNDEFINED_HELPER"
    );

    rule_err!(
        apply_when_wrong_arg_count_then_error,
        "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value, value);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallWrongArgCount]
    );

    /// A formal call may omit any `VAR_INPUT`, which starts from its initial
    /// value, the declared one or its type's; a call with no inputs omits
    /// them all (#2552).
    #[spec_test(REQ_CB_analyzer_057)]
    #[rstest]
    #[case::omits_one_with_initial_value("SCALE(x := 4)")]
    #[case::omits_one_without_initial_value("SCALE(k := 5)")]
    #[case::omits_several_out_of_order("SCALE(off := 1, x := 4)")]
    #[case::omits_all("SCALE()")]
    #[case::omits_all_but_assigns_output("SCALE(q => o)")]
    fn apply_when_user_function_formal_call_omits_input_then_ok(#[case] call: &str) {
        let program = format!(
            "
FUNCTION SCALE : INT
VAR_INPUT x : INT; k : INT := 3; off : INT := -1; END_VAR
VAR_OUTPUT q : INT; END_VAR
    q := x;
    SCALE := x * k + off;
END_FUNCTION

PROGRAM main
VAR y : INT; o : INT; END_VAR
    y := {call};
END_PROGRAM"
        );
        let diagnostics = rule_diagnostics(apply, &program, &CompilerOptions::default());
        assert!(diagnostics.is_empty(), "{call}: {diagnostics:?}");
    }

    /// A `VAR_IN_OUT` names the caller's variable, so a formal call that
    /// omits one has no value to take: one P4018 for each.
    #[spec_test(REQ_CB_analyzer_058)]
    #[test]
    fn apply_when_user_function_formal_call_omits_in_outs_then_error_for_each() {
        let program = "
FUNCTION SWAP : BOOL
VAR_INPUT go : BOOL := TRUE; END_VAR
VAR_IN_OUT a : DINT; b : DINT; END_VAR
VAR t : DINT; END_VAR
    t := a;
    a := b;
    b := t;
    SWAP := go;
END_FUNCTION

PROGRAM main
VAR r : BOOL; END_VAR
    r := SWAP(go := FALSE);
END_PROGRAM";
        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());
        let missing: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(Problem::FunctionCallWrongArgCount.code(), diagnostic.code);
                diagnostic
                    .described
                    .iter()
                    .find(|context| context.starts_with("missing in-out"))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(vec!["missing in-out=a", "missing in-out=b"], missing);
    }

    rule_err!(
        /// A standard function's inputs are passed by position, so a formal
        /// call of one still supplies every input it declares.
        #[spec_test(REQ_CB_analyzer_059)]
        apply_when_standard_function_formal_call_omits_input_then_wrong_arg_count,
        "
PROGRAM main
VAR r : INT; b : INT; END_VAR
    r := LIMIT(IN := b, MX := 5);
END_PROGRAM",
        [Problem::FunctionCallWrongArgCount]
    );

    rule_ok!(
        apply_when_abs_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    value : INT;
END_VAR
    result := ABS(value);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_sqrt_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : REAL;
END_VAR
    result := SQRT(value);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_min_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MIN(a, b);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_max_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MAX(a, b);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_limit_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    low : INT;
    value : INT;
    high : INT;
END_VAR
    result := LIMIT(low, value, high);
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_limit_called_with_wrong_arg_count_then_error,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := LIMIT(a, b);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallWrongArgCount]
    );

    rule_ok!(
        apply_when_expt_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    base : INT;
    exp : INT;
END_VAR
    result := EXPT(base, exp);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_mux_called_with_3_args_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MUX(0, a, b);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_mux_called_with_5_args_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
END_VAR
    result := MUX(2, a, b, c, d);
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_mux_called_with_too_few_args_then_error,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
END_VAR
    result := MUX(0, a);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallWrongArgCount]
    );

    rule_ok!(
        apply_when_mux_called_with_17_args_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
    e : INT;
    f : INT;
    g : INT;
    h : INT;
    i : INT;
    j : INT;
    k : INT;
    l : INT;
    m : INT;
    n : INT;
    o : INT;
    p : INT;
END_VAR
    result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p);
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_mux_called_with_18_args_then_error,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
    c : INT;
    d : INT;
    e : INT;
    f : INT;
    g : INT;
    h : INT;
    i : INT;
    j : INT;
    k : INT;
    l : INT;
    m : INT;
    n : INT;
    o : INT;
    p : INT;
    q : INT;
END_VAR
    result := MUX(0, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallWrongArgCount]
    );

    rule_err!(
        /// A positional call supplies every input, even one that declares an
        /// initial value.
        #[spec_test(REQ_CB_analyzer_059)]
        apply_when_too_few_args_then_error,
        "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT := 2;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallWrongArgCount]
    );

    // Note: MOD is excluded because the parser treats it as a keyword (the MOD operator).
    // MOD(a, b) requires parser changes to allow keywords in function call position.
    rule_ok!(
        apply_when_arithmetic_function_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := ADD(a, b);
    result := SUB(a, b);
    result := MUL(a, b);
    result := DIV(a, b);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_comparison_function_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : INT;
    b : INT;
END_VAR
    result := GT(a, b);
    result := GE(a, b);
    result := EQ(a, b);
    result := LE(a, b);
    result := LT(a, b);
    result := NE(a, b);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_bit_string_conversion_called_then_ok,
        "
FUNCTION_BLOCK CALLER
VAR
    b : BYTE;
    w : WORD;
    d : DWORD;
    l : LWORD;
END_VAR
    w := BYTE_TO_WORD(b);
    d := BYTE_TO_DWORD(b);
    l := BYTE_TO_LWORD(b);
    b := WORD_TO_BYTE(w);
    d := WORD_TO_DWORD(w);
    l := WORD_TO_LWORD(w);
    b := DWORD_TO_BYTE(d);
    w := DWORD_TO_WORD(d);
    l := DWORD_TO_LWORD(d);
    b := LWORD_TO_BYTE(l);
    w := LWORD_TO_WORD(l);
    d := LWORD_TO_DWORD(l);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_shl_with_byte_to_word_conversion_then_ok,
        "
FUNCTION MY_SHIFT : WORD
VAR_INPUT
    B : BYTE;
END_VAR
    MY_SHIFT := SHL(BYTE_TO_WORD(B), 8);
END_FUNCTION

PROGRAM main
VAR
    result : WORD;
END_VAR
    result := MY_SHIFT(B := BYTE#16#AB);
END_PROGRAM"
    );

    // A call is either formal or non-formal; with a mix, the arity check
    // below it would count a misreading of the arguments, so P4001 is the
    // only diagnostic (#1816).
    rule_err!(
        apply_when_function_call_mixes_positional_and_named_then_error,
        "
FUNCTION F : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    F := a - b;
END_FUNCTION

PROGRAM main
VAR
    x : INT;
END_VAR
    x := F(1, b := 2);
END_PROGRAM",
        [Problem::FunctionCallMixedArgTypes]
    );

    rule_ok!(
        apply_when_function_call_named_with_output_then_ok,
        "
FUNCTION F : INT
VAR_INPUT
    a : INT;
END_VAR
VAR_OUTPUT
    q : INT;
END_VAR
    q := a;
    F := a;
END_FUNCTION

PROGRAM main
VAR
    x : INT;
    y : INT;
END_VAR
    x := F(a := 1, q => y);
END_PROGRAM"
    );

    // A VAR_IN_OUT parameter takes an argument like a VAR_INPUT does, in
    // either declaration order and with either call form (#1658).
    #[rstest]
    #[case::in_out_last_named(
        "VAR_INPUT n : DINT; END_VAR VAR_IN_OUT data : DINT; END_VAR",
        "ADD_N(n := 42, data := x)"
    )]
    #[case::in_out_last_positional(
        "VAR_INPUT n : DINT; END_VAR VAR_IN_OUT data : DINT; END_VAR",
        "ADD_N(42, x)"
    )]
    #[case::in_out_first_named(
        "VAR_IN_OUT data : DINT; END_VAR VAR_INPUT n : DINT; END_VAR",
        "ADD_N(n := 42, data := x)"
    )]
    #[case::in_out_first_positional(
        "VAR_IN_OUT data : DINT; END_VAR VAR_INPUT n : DINT; END_VAR",
        "ADD_N(x, 42)"
    )]
    fn apply_when_function_has_in_out_then_counts_in_out_argument(
        #[case] params: &str,
        #[case] call: &str,
    ) {
        let program = format!(
            "
FUNCTION ADD_N : DINT
{params}
    data := data + n;
    ADD_N := data;
END_FUNCTION

PROGRAM main
VAR
    x : DINT := 100;
    result : DINT;
END_VAR
    result := {call};
END_PROGRAM"
        );
        let diagnostics = rule_diagnostics(apply, &program, &CompilerOptions::default());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    rule_err!(
        apply_when_in_out_argument_missing_then_error,
        "
FUNCTION ADD_N : DINT
VAR_INPUT n : DINT; END_VAR
VAR_IN_OUT data : DINT; END_VAR
    ADD_N := data + n;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
END_VAR
    result := ADD_N(42);
END_PROGRAM",
        [Problem::FunctionCallWrongArgCount]
    );

    /// A program that stores the call `call` in a `STRING`.
    fn concat_program(call: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    s : STRING;
END_VAR
    s := {call};
END_PROGRAM"
        )
    }

    /// CODESYS and TwinCAT declare `CONCAT` with two inputs, so a third is
    /// P4018, with help on writing the call in the dialect; the IEC 61131-3
    /// dialects and RuSTy declare it extensible.
    #[spec_test(REQ_CB_analyzer_056)]
    fn apply_when_concat_of_three_then_p4018_where_concat_takes_two() {
        use ironplc_parser::options::Dialect;

        let program = concat_program("CONCAT('a', 'b', 'c')");
        for dialect in Dialect::ALL {
            let options = CompilerOptions::from_dialect(*dialect);
            let diagnostics = rule_diagnostics(apply, &program, &options);
            if matches!(dialect, Dialect::Codesys | Dialect::TwinCat) {
                assert_eq!(
                    diagnostic_codes(&diagnostics),
                    codes(&[Problem::FunctionCallWrongArgCount]),
                    "{dialect}"
                );
                assert!(
                    diagnostics[0]
                        .help()
                        .iter()
                        .any(|help| help.contains("CONCAT(CONCAT(a, b), c)")),
                    "{dialect}: {diagnostics:?}"
                );
            } else {
                assert_eq!(diagnostic_codes(&diagnostics), codes(&[]), "{dialect}");
            }
        }
    }

    #[rstest]
    #[case::two_inputs("CONCAT('a', 'b')", &[])]
    #[case::one_input("CONCAT('a')", &[Problem::FunctionCallWrongArgCount])]
    fn apply_when_two_input_concat_then_counts_as_two_input_function(
        #[case] call: &str,
        #[case] expected: &[Problem],
    ) {
        let options = CompilerOptions {
            allow_extensible_concat: false,
            ..CompilerOptions::default()
        };
        let diagnostics = rule_diagnostics(apply, &concat_program(call), &options);

        assert_eq!(diagnostic_codes(&diagnostics), codes(expected));
        // Too few inputs is no call the extensible CONCAT would take either.
        assert!(
            diagnostics.iter().all(|d| d.help().is_empty()),
            "{diagnostics:?}"
        );
    }
}
