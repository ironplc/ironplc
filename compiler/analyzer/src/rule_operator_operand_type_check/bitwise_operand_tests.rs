//! Tests for the operands of `AND`, `OR`, `XOR`, `AND_THEN` and `OR_ELSE`:
//! they must have a type every operand widens to. A narrower bit string
//! widens to a wider one; a `BOOL` widens to none, so a `BOOL` with a bit
//! string is P4049 in every dialect.

use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::test_helpers::rule_codes;

/// A program declaring two `BOOL`s, a `BYTE`, a `WORD` and an `LWORD`,
/// whose body is `body`.
fn program(body: &str) -> String {
    format!(
        "
PROGRAM main
VAR
    b : BOOL;
    c : BOOL;
    y : BYTE;
    w : WORD;
    lw : LWORD;
END_VAR
    {body}
END_PROGRAM"
    )
}

fn codes_for(body: &str, options: &CompilerOptions) -> Vec<String> {
    rule_codes(super::apply, &program(body), options)
}

fn p4049() -> Vec<String> {
    vec![Problem::OperatorOperandTypeMismatch.code().to_string()]
}

#[spec_test(REQ_IC_analyzer_122)]
#[rstest]
#[case::and("c := b AND w;")]
#[case::bit_string_on_the_left("w := w OR b;")]
#[case::xor("w := w XOR b;")]
#[case::function_form("w := XOR(w, b);")]
#[case::extensible_function_form("lw := OR(y, lw, b);")]
fn apply_when_bool_with_bit_string_then_p4049(#[case] body: &str) {
    for dialect in Dialect::ALL {
        let options = CompilerOptions::from_dialect(*dialect);
        assert_eq!(codes_for(body, &options), p4049(), "{dialect:?}");
    }
}

#[spec_test(REQ_IC_analyzer_122)]
#[rstest]
#[case::and_then("c := b AND_THEN w;")]
#[case::or_else("c := w OR_ELSE b;")]
fn apply_when_short_circuit_bool_with_bit_string_then_p4049(#[case] body: &str) {
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    assert_eq!(codes_for(body, &options), p4049());
}

#[spec_test(REQ_IC_analyzer_122)]
#[rstest]
#[case::booleans("c := b AND c;")]
#[case::bit_strings_of_two_widths("lw := w OR lw;")]
#[case::function_form_of_bit_strings("lw := AND(y, w, lw);")]
#[case::booleans_function_form("c := XOR(b, c);")]
fn apply_when_operands_widen_to_one_type_then_ok(#[case] body: &str) {
    let codes = codes_for(body, &CompilerOptions::default());
    assert!(codes.is_empty(), "{codes:?}");
}

rule_err_at!(
    /// The expression is labelled whole, naming both types.
    apply_when_bool_with_bit_string_then_p4049_at_expression,
    "
PROGRAM main
VAR
    b : BOOL;
    w : WORD;
END_VAR
    w := b AND w;
END_PROGRAM",
    Problem::OperatorOperandTypeMismatch,
    "b AND w"
);
