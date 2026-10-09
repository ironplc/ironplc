//! Tests for the conversions the pass records for the operands of a typed
//! time or date operation: to the width its routine computes at.

use ironplc_parser::options::Dialect;

use super::*;

/// The operands of every operator expression and call in a library, in
/// source order outermost first, each as [`describe`] shows it.
struct Operands<'a> {
    types: &'a TypeEnvironment,
    operands: Vec<Vec<String>>,
}

impl Visitor<Infallible> for Operands<'_> {
    type Value = ();

    fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
        match &node.kind {
            ExprKind::BinaryOp(binary) => {
                let operands = [&binary.left, &binary.right];
                self.operands
                    .push(operands.map(|e| describe(self.types, e)).to_vec());
            }
            ExprKind::Function(func) => {
                let inputs = func.param_assignment.iter().filter_map(|p| p.input_expr());
                self.operands
                    .push(inputs.map(|e| describe(self.types, e)).collect());
            }
            _ => {}
        }
        node.recurse_visit(self)
    }
}

/// The operands of `x := <value>` for an `x` of type `target`, in a program
/// of the third edition declaring a variable of each type the tests use.
fn time_operands(target: &str, value: &str) -> Vec<Vec<String>> {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; t : TIME; lt : LTIME; da : DATE; lda : LDATE; dt1 : DT; ldt1 : LDT;
             si : SINT; d : DINT; ud : UDINT; l : LINT; r : REAL; lr : LREAL; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
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

#[spec_test(REQ_IC_analyzer_096)]
#[rstest]
#[case::short_on_right("LTIME", "lt + t", ["LTIME", "TIME->LTIME"])]
#[case::short_on_left("LTIME", "t + lt", ["TIME->LTIME", "LTIME"])]
#[case::function_form("LTIME", "ADD(t, lt)", ["TIME->LTIME", "LTIME"])]
#[case::typed_call("LTIME", "ADD_LTIME(lt, t)", ["LTIME", "TIME->LTIME"])]
#[case::date_difference("LTIME", "lda - da", ["LDATE", "DATE->LDATE"])]
#[case::typed_date_difference("LTIME", "SUB_LDATE_LDATE(lda, da)", ["LDATE", "DATE->LDATE"])]
#[case::date_and_time_offset("LDT", "ldt1 + t", ["LDATE_AND_TIME", "TIME->LTIME"])]
#[case::same_width("TIME", "t + t", ["TIME", "TIME"])]
fn apply_when_short_operand_of_long_time_operation_then_converted_to_long_type(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: [&str; 2],
) {
    assert_eq!(time_operands(target, value), operands(&[&expected]));
}

#[spec_test(REQ_IC_analyzer_097)]
#[rstest]
#[case::long_by_dint("LTIME", "lt * d", "DINT->LINT")]
#[case::long_by_udint("LTIME", "MUL_LTIME(lt, ud)", "UDINT->LINT")]
#[case::long_by_lint("LTIME", "lt / l", "LINT")]
#[case::long_by_real("LTIME", "lt * r", "REAL->LREAL")]
#[case::long_by_literal("LTIME", "lt / 2", "LINT")]
#[case::short_by_lint("TIME", "t * l", "LINT->LREAL")]
#[case::typed_short_by_lint("TIME", "MUL_TIME(t, l)", "LINT->LREAL")]
#[case::short_by_sint("TIME", "t * si", "SINT")]
#[case::short_by_real("TIME", "t * r", "REAL")]
fn apply_when_time_scaled_by_number_then_number_converted_to_routine_width(
    #[case] target: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    let operands = time_operands(target, value);
    assert_eq!(operands[0][1], expected);
}

#[spec_test(REQ_IC_analyzer_098)]
#[test]
fn apply_when_typed_fold_has_three_inputs_then_written_as_the_calls_it_folds_to() {
    assert_eq!(
        time_operands("LTIME", "ADD(t, t, lt)"),
        vec![
            vec!["TIME->LTIME".to_string(), "LTIME".to_string()],
            vec!["TIME".to_string(), "TIME".to_string()],
        ]
    );
}
