//! Tests for the conversion the pass records for a value its context
//! computes at a fixed type: an array subscript at `DINT`, the count of a
//! shift or rotate at `LINT` or `DINT`, and the position or length of a
//! string function at `DINT`.

use super::*;

/// Each array subscript, each shift or rotate count and each position or
/// length of a string function in a library, in source order, as
/// [`describe`] shows it.
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
        for index in string_positions(node.name.original()) {
            if let Some(position) = node
                .param_assignment
                .get(*index)
                .and_then(|p| p.input_expr())
            {
                self.values.push(describe(self.types, position));
            }
        }
        node.recurse_visit(self)
    }
}

/// The indices of the inputs of the string function `name` that are a
/// position or a length, rather than a string.
fn string_positions(name: &str) -> &'static [usize] {
    match name {
        "LEFT" | "RIGHT" => &[1],
        "MID" | "DELETE" => &[1, 2],
        "INSERT" => &[2],
        "REPLACE" => &[2, 3],
        _ => &[],
    }
}

/// The subscripts, counts, positions and lengths of `body`, in a program
/// declaring a variable of each type the tests use.
fn fixed_type_values(body: &str) -> Vec<String> {
    let source = format!(
        "TYPE SMALL : LINT(0..3); END_TYPE
         PROGRAM main
         VAR a : ARRAY[0..3] OF INT; x : INT; s : SINT; d : DINT; l : LINT; u : UINT;
             ud : UDINT; ul : ULINT; w : WORD; dw : DWORD; lw : LWORD; st : STRING[10];
             r : SMALL; END_VAR
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
#[case::lint_variable("x := a[l];", "LINT->DINT checked")]
#[case::lint_arithmetic("x := a[d + l];", "LINT->DINT checked")]
#[case::assignment_target("a[l] := 5;", "LINT->DINT checked")]
#[case::narrower_integer("x := a[s];", "SINT")]
#[case::unsigned_integer("x := a[u];", "UINT->DINT")]
#[case::unsigned_of_dint_width("x := a[ud];", "UDINT->DINT checked")]
#[case::unsigned_64_bit("x := a[ul];", "ULINT->DINT checked")]
#[case::bit_string("x := a[w];", "WORD->DINT")]
#[case::bit_string_of_dint_width("x := a[dw];", "DWORD->DINT checked")]
#[case::bit_string_64_bit("x := a[lw];", "LWORD->DINT checked")]
#[case::subrange_of_lint("x := a[r];", "LINT->DINT checked")]
#[case::dint("x := a[d];", "DINT")]
fn apply_when_subscript_of_other_operation_type_then_converted_to_dint(
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
#[case::ulint_count_of_lword("lw := SHR(lw, ul);", "ULINT->LINT")]
#[case::dint_count_of_lword_rotated("lw := ROR(lw, d);", "DINT->LINT")]
fn apply_when_shift_count_of_other_operation_type_then_converted_to_count_type(
    #[case] body: &str,
    #[case] expected: &str,
) {
    assert_eq!(fixed_type_values(body), vec![expected]);
}

#[spec_test(REQ_IC_analyzer_104)]
#[rstest]
#[case::lint_length("st := LEFT(st, l);", &["LINT->DINT"])]
#[case::lint_arithmetic_length("st := RIGHT(st, d + l);", &["LINT->DINT"])]
#[case::length_and_position("st := MID(st, l, d);", &["LINT->DINT", "DINT"])]
#[case::ulint_length("st := DELETE(st, ul, l);", &["ULINT->DINT", "LINT->DINT"])]
#[case::insert_position("st := INSERT(st, 'ab', l);", &["LINT->DINT"])]
#[case::replace_length_and_position("st := REPLACE(st, 'ab', d, l);", &["DINT", "LINT->DINT"])]
#[case::narrower_integer("st := LEFT(st, s);", &["SINT"])]
#[case::unsigned_integer("st := LEFT(st, u);", &["UINT->DINT"])]
fn apply_when_string_position_of_other_operation_type_then_converted_to_dint(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(fixed_type_values(body), expected);
}
