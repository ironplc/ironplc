//! Tests for the conversion the pass records for a value its context
//! computes at a fixed type: an array subscript at `DINT`, and the count of
//! a shift or rotate at `LINT` or `DINT`.

use super::*;

/// Each array subscript and each shift or rotate count in a library, in
/// source order, as [`describe`] shows it.
struct FixedTypeValues<'a> {
    types: &'a TypeEnvironment,
    values: Vec<String>,
}

impl Visitor<Infallible> for FixedTypeValues<'_> {
    type Value = ();

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<(), Infallible> {
        for subscript in &node.subscripts {
            self.values.push(describe(self.types, subscript));
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
        if ["SHL", "SHR", "ROL", "ROR"].contains(&node.name.original().as_str()) {
            if let Some(count) = node.param_assignment.get(1).and_then(|p| p.input_expr()) {
                self.values.push(describe(self.types, count));
            }
        }
        node.recurse_visit(self)
    }
}

/// The subscripts and counts of `body`, in a program declaring a variable
/// of each type the tests use.
fn fixed_type_values(body: &str) -> Vec<String> {
    let source = format!(
        "PROGRAM main
         VAR a : ARRAY[0..3] OF INT; x : INT; s : SINT; d : DINT; l : LINT; ul : ULINT;
             dw : DWORD; lw : LWORD; END_VAR
         {body}
         END_PROGRAM"
    );
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(&source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = FixedTypeValues {
        types: context.types(),
        values: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.values
}

#[spec_test(REQ_IC_analyzer_100)]
#[rstest]
#[case::lint_variable("x := a[l];", "LINT->DINT")]
#[case::lint_arithmetic("x := a[d + l];", "LINT->DINT")]
#[case::assignment_target("a[l] := 5;", "LINT->DINT")]
#[case::narrower_integer("x := a[s];", "SINT")]
#[case::dint("x := a[d];", "DINT")]
fn apply_when_subscript_of_other_width_then_converted_to_dint(
    #[case] body: &str,
    #[case] expected: &str,
) {
    assert_eq!(fixed_type_values(body), vec![expected]);
}

#[spec_test(REQ_IC_analyzer_101)]
#[rstest]
#[case::lint_count_of_dword("dw := SHL(dw, l);", "LINT->DINT")]
#[case::dint_count_of_lword("lw := SHL(lw, d);", "DINT->LINT")]
#[case::sint_count_of_dword("dw := ROL(dw, s);", "SINT")]
#[case::ulint_count_of_lword("lw := SHR(lw, ul);", "ULINT")]
#[case::dint_count_of_lword_rotated("lw := ROR(lw, d);", "DINT->LINT")]
fn apply_when_shift_count_of_other_width_then_converted_to_count_type(
    #[case] body: &str,
    #[case] expected: &str,
) {
    assert_eq!(fixed_type_values(body), vec![expected]);
}
