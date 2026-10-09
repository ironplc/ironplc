//! Tests for the conversions the pass records for `AND`, `OR` and `XOR`: an
//! operand of another width is converted to the type of the operation, and
//! the operation's result to a wider target.

use super::*;

/// The variables the programs of these tests declare.
const VARS: &str = "b : BYTE; w : WORD; d : DWORD; e : DWORD; lw : LWORD; g : BOOL; h : BOOL;";

/// The operands of every `AND`, `OR` and `XOR` operator expression in a
/// program assigning `value` to a variable of type `target`, each as
/// [`describe`] shows it.
fn bitwise_operands(target: &str, value: &str) -> Vec<Vec<String>> {
    struct Operands<'a> {
        types: &'a TypeEnvironment,
        operands: Vec<Vec<String>>,
    }
    impl Visitor<Infallible> for Operands<'_> {
        type Value = ();
        fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<(), Infallible> {
            if matches!(node.op, CompareOp::And | CompareOp::Or | CompareOp::Xor) {
                let operands = [&node.left, &node.right];
                self.operands
                    .push(operands.map(|e| describe(self.types, e)).to_vec());
            }
            node.recurse_visit(self)
        }
    }
    let options = CompilerOptions::default();
    let source = arithmetic_program(target, VARS, value);
    let library = ironplc_parser::parse_program(&source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = Operands {
        types: context.types(),
        operands: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.operands
}

#[spec_test(REQ_IC_analyzer_082)]
#[rstest]
#[case::narrower_right("lw OR w", ["LWORD", "WORD->LWORD"])]
#[case::narrower_left("w OR lw", ["WORD->LWORD", "LWORD"])]
#[case::xor("d XOR lw", ["DWORD->LWORD", "LWORD"])]
fn apply_when_bitwise_operand_of_another_width_then_converted_to_operation_type(
    #[case] value: &str,
    #[case] expected: [&str; 2],
) {
    assert_eq!(bitwise_operands("LWORD", value), operands(&[&expected]));
}

#[spec_test(REQ_IC_analyzer_082)]
#[rstest]
#[case::same_width_bit_strings("w AND d", "DWORD", ["WORD", "DWORD"])]
#[case::booleans("g AND h", "BOOL", ["BOOL", "BOOL"])]
fn apply_when_bitwise_operands_share_a_width_then_unchanged(
    #[case] value: &str,
    #[case] target: &str,
    #[case] expected: [&str; 2],
) {
    assert_eq!(bitwise_operands(target, value), operands(&[&expected]));
}

#[spec_test(REQ_IC_analyzer_082)]
#[rstest]
#[case::and("AND(w, lw)", "AND", &["WORD->LWORD", "LWORD"])]
#[case::extensible_or("OR(b, d, lw)", "OR", &["BYTE->LWORD", "DWORD->LWORD", "LWORD"])]
fn apply_when_bitwise_function_form_then_inputs_converted_as_operator_operands(
    #[case] value: &str,
    #[case] name: &str,
    #[case] expected: &[&str],
) {
    let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        call_arguments(&arithmetic_program("LWORD", VARS, value), name),
        expected
    );
}

#[spec_test(REQ_IC_analyzer_083)]
#[rstest]
#[case::operator("LWORD", "d OR e", "DWORD->LWORD")]
#[case::function_form("LWORD", "OR(d, e)", "DWORD->LWORD")]
#[case::same_width("DWORD", "d OR e", "DWORD")]
fn apply_when_bitwise_result_assigned_then_converted_to_target_type(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        assigned_values(&arithmetic_program(target, VARS, value)),
        vec![expected.to_string()]
    );
}
