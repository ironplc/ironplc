//! Unit tests for `xform_insert_implicit_conversions`.

use std::convert::Infallible;

use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::CompilerOptions;

use rstest::rstest;
use spec_test_macro::spec_test;

use crate::stages::analyze;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

/// The operands of every comparison in a library, in source order, each as
/// the type it is compared at: `DINT->LINT` for a `DINT` converted to
/// `LINT`, `LINT` for an operand compared as it is.
struct ComparisonOperands<'a> {
    types: &'a TypeEnvironment,
    operands: Vec<[String; 2]>,
}

impl ComparisonOperands<'_> {
    fn describe(&self, expr: &Expr) -> String {
        describe(self.types, expr)
    }
}

/// An operand as the type it is operated at: `DINT->LINT` for a `DINT`
/// converted to `LINT`, `LINT` for an operand operated on as it is.
pub(super) fn describe(types: &TypeEnvironment, expr: &Expr) -> String {
    let name = |expr: &Expr| {
        expr.expr_type
            .as_ref()
            .and_then(|t| operand_type_name(types, t))
            .map_or("?".to_string(), |name| name.to_string().to_uppercase())
    };
    match &expr.kind {
        ExprKind::ImplicitConversion(inner) => format!("{}->{}", name(inner), name(expr)),
        _ => name(expr),
    }
}

impl Visitor<Infallible> for ComparisonOperands<'_> {
    type Value = ();

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<(), Infallible> {
        if node.op.is_comparison() {
            let pair = [self.describe(&node.left), self.describe(&node.right)];
            self.operands.push(pair);
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
        if let [ParamAssignmentKind::PositionalInput(left), ParamAssignmentKind::PositionalInput(right)] =
            node.param_assignment.as_slice()
        {
            let pair = [self.describe(&left.expr), self.describe(&right.expr)];
            self.operands.push(pair);
        }
        node.recurse_visit(self)
    }
}

/// Analyzes `source`, which must be free of diagnostics, and returns the
/// operands of its comparisons.
fn comparison_operands(source: &str) -> Vec<[String; 2]> {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = ComparisonOperands {
        types: context.types(),
        operands: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.operands
}

/// The operands of every arithmetic operator expression and function form in
/// a library, in source order outermost first, each as [`describe`] shows it.
struct ArithmeticOperands<'a> {
    types: &'a TypeEnvironment,
    operands: Vec<Vec<String>>,
}

impl Visitor<Infallible> for ArithmeticOperands<'_> {
    type Value = ();

    fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
        match &node.kind {
            ExprKind::BinaryOp(binary) => {
                let operands = [&binary.left, &binary.right];
                self.operands
                    .push(operands.map(|e| describe(self.types, e)).to_vec());
            }
            ExprKind::Function(func)
                if ARITHMETIC_FORMS.contains(&func.name.original().as_str()) =>
            {
                let inputs = func.param_assignment.iter().filter_map(|p| p.input_expr());
                self.operands
                    .push(inputs.map(|e| describe(self.types, e)).collect());
            }
            _ => {}
        }
        node.recurse_visit(self)
    }
}

const ARITHMETIC_FORMS: [&str; 5] = ["ADD", "SUB", "MUL", "DIV", "MOD"];

/// Analyzes `source`, which must be free of diagnostics, and returns the
/// operands of its arithmetic.
fn arithmetic_operands(source: &str) -> Vec<Vec<String>> {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = ArithmeticOperands {
        types: context.types(),
        operands: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.operands
}

/// The value of every assignment in a library, in source order, each as
/// [`describe`] shows it.
struct AssignedValues<'a> {
    types: &'a TypeEnvironment,
    values: Vec<String>,
}

impl Visitor<Infallible> for AssignedValues<'_> {
    type Value = ();

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.values.push(describe(self.types, &node.value));
        node.recurse_visit(self)
    }
}

/// Analyzes `source`, which must be free of diagnostics, and returns the
/// values of its assignments.
fn assigned_values(source: &str) -> Vec<String> {
    assigned_values_with(source, &CompilerOptions::default())
}

/// [`assigned_values`] under `options`.
pub(super) fn assigned_values_with(source: &str, options: &CompilerOptions) -> Vec<String> {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = AssignedValues {
        types: context.types(),
        values: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.values
}

/// A program declaring `VAR <vars> END_VAR` whose body is `x := <expr>;` for
/// an `x` of type `target`.
fn arithmetic_program(target: &str, vars: &str, expr: &str) -> String {
    format!("PROGRAM main VAR x : {target}; {vars} END_VAR x := {expr}; END_PROGRAM")
}

fn operands(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|operand| operand.to_string()).collect())
        .collect()
}

/// A program declaring `VAR <vars> END_VAR` whose body is `b := <compare>;`.
fn program(vars: &str, compare: &str) -> String {
    format!("PROGRAM main VAR b : BOOL; {vars} END_VAR b := {compare}; END_PROGRAM")
}

fn pair(left: &str, right: &str) -> Vec<[String; 2]> {
    vec![[left.to_string(), right.to_string()]]
}

#[spec_test(REQ_IC_analyzer_001)]
#[test]
fn apply_when_left_operand_wider_then_right_variable_converted() {
    let source = program("d : DINT; l : LINT;", "l > d");
    assert_eq!(comparison_operands(&source), pair("LINT", "DINT->LINT"));
}

#[test]
fn apply_when_integer_variable_compared_with_real_then_converted_to_real() {
    let source = program("i : INT; r : REAL;", "r >= i");
    assert_eq!(comparison_operands(&source), pair("REAL", "INT->REAL"));
}

#[spec_test(REQ_IC_analyzer_010)]
#[test]
fn apply_when_right_operand_wider_then_left_converted() {
    let source = program("d : DINT; l : LINT;", "d < l");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[spec_test(REQ_IC_analyzer_011)]
#[test]
fn apply_when_neither_widens_then_right_converted_to_left() {
    let source = program("d : DINT; u : UDINT;", "d < u");
    assert_eq!(comparison_operands(&source), pair("DINT", "UDINT->DINT"));
}

/// A named subrange is compared at its base type, so it is the `INT` that
/// widens, not the `LINT` that narrows to it.
#[spec_test(REQ_IC_analyzer_092)]
#[test]
fn apply_when_named_subrange_and_wider_integer_then_subrange_converted_to_wider() {
    let source = "TYPE Small : INT (0..10); END_TYPE
        PROGRAM main VAR b : BOOL; s : Small; l : LINT; END_VAR b := s < l; END_PROGRAM";
    assert_eq!(comparison_operands(source), pair("INT->LINT", "LINT"));
}

#[spec_test(REQ_IC_analyzer_003)]
#[test]
fn apply_when_literal_on_right_then_literal_takes_operand_type() {
    let source = program("l : LINT;", "l > 1");
    assert_eq!(comparison_operands(&source), pair("LINT", "LINT"));
}

#[test]
fn apply_when_literal_on_left_then_literal_takes_operand_type() {
    let source = program("l : LINT;", "1 < l");
    assert_eq!(comparison_operands(&source), pair("LINT", "LINT"));
}

#[test]
fn apply_when_operands_same_type_then_unchanged() {
    let source = program("d1 : DINT; d2 : DINT;", "d1 = d2");
    assert_eq!(comparison_operands(&source), pair("DINT", "DINT"));
}

#[spec_test(REQ_IC_analyzer_004)]
#[test]
fn apply_when_alias_of_operand_type_then_unchanged() {
    let source = "TYPE MyInt : INT; END_TYPE
        PROGRAM main VAR b : BOOL; a : MyInt; i : INT; END_VAR b := a = i; END_PROGRAM";
    assert_eq!(comparison_operands(source), pair("INT", "INT"));
}

#[spec_test(REQ_IC_analyzer_012)]
#[test]
fn apply_when_call_of_narrower_type_then_converted() {
    let source = program("d : DINT; l : LINT;", "ABS(d) < l");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[test]
fn apply_when_short_and_long_time_then_short_converted() {
    let source = program("t : TIME; lt : LTIME;", "t < lt");
    assert_eq!(comparison_operands(&source), pair("TIME->LTIME", "LTIME"));
}

#[spec_test(REQ_IC_analyzer_013)]
#[test]
fn apply_when_function_form_then_inputs_converted() {
    let source = program("d : DINT; l : LINT;", "GT(d, l)");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[spec_test(REQ_IC_analyzer_007)]
#[test]
fn apply_when_strings_then_unchanged() {
    let source = program("s1 : STRING; s2 : STRING[10];", "s1 = s2");
    let operands = comparison_operands(&source);
    assert!(
        operands
            .iter()
            .flatten()
            .all(|operand| !operand.contains("->")),
        "{operands:?}"
    );
}

#[test]
fn apply_when_bitwise_operator_then_operands_unchanged() {
    let options = CompilerOptions::default();
    let source =
        "PROGRAM main VAR w : WORD; d : DWORD; r : DWORD; END_VAR r := w AND d; END_PROGRAM";
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, _context) = analyze(&[&library], &options).unwrap();
    assert!(!format!("{library:?}").contains("ImplicitConversion"));
}

#[spec_test(REQ_IC_analyzer_008)]
#[test]
fn apply_when_nested_in_condition_then_conversion_spans_operand() {
    let options = CompilerOptions::default();
    let source =
        "PROGRAM main VAR d : DINT; l : LINT; END_VAR IF l > d THEN d := 1; END_IF; END_PROGRAM";
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, _context) = analyze(&[&library], &options).unwrap();

    struct Conversions(Vec<String>);
    impl Visitor<Infallible> for Conversions {
        type Value = ();
        fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
            if let ExprKind::ImplicitConversion(inner) = &node.kind {
                assert_eq!(node.span, inner.span);
                self.0.push(node.to_string());
            }
            node.recurse_visit(self)
        }
    }
    let mut conversions = Conversions(vec![]);
    let Ok(()) = conversions.walk(&library);
    // Written as the operand it converts: it is not source syntax.
    assert_eq!(conversions.0, vec!["d"]);
}

#[spec_test(REQ_IC_analyzer_009)]
#[test]
fn apply_when_typed_literal_out_of_range_of_narrower_operand_then_rules_still_report() {
    let options = CompilerOptions::default();
    let source = program("s : SINT;", "DINT#300 < s");
    let library = ironplc_parser::parse_program(&source, &FileId::default(), &options).unwrap();
    let (_library, context) = analyze(&[&library], &options).unwrap();
    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["P2026"]);
}

#[spec_test(REQ_IC_analyzer_020)]
#[test]
fn apply_when_arithmetic_operand_narrower_then_converted_to_result_type() {
    let source = arithmetic_program("REAL", "i : INT; r : REAL;", "i + r");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["INT->REAL", "REAL"]])
    );
}

#[spec_test(REQ_IC_analyzer_021)]
#[test]
fn apply_when_arithmetic_operand_is_literal_then_literal_takes_result_type() {
    let source = arithmetic_program("LINT", "l : LINT;", "l + 1");
    assert_eq!(arithmetic_operands(&source), operands(&[&["LINT", "LINT"]]));
}

#[spec_test(REQ_IC_analyzer_022)]
#[test]
fn apply_when_arithmetic_operands_share_a_width_then_unchanged() {
    let source = arithmetic_program("INT", "i : INT; s : SINT;", "i + s");
    assert_eq!(arithmetic_operands(&source), operands(&[&["INT", "SINT"]]));
}

#[spec_test(REQ_IC_analyzer_023)]
#[test]
fn apply_when_arithmetic_pair_has_typed_overload_then_unchanged() {
    let source = arithmetic_program("TIME", "t1 : TIME; t2 : TIME;", "t1 + t2");
    assert_eq!(arithmetic_operands(&source), operands(&[&["TIME", "TIME"]]));
}

#[spec_test(REQ_IC_analyzer_024)]
#[test]
fn apply_when_function_form_then_inputs_converted_as_operator_operands() {
    let source = arithmetic_program("REAL", "i : INT; r : REAL;", "ADD(i, r)");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["INT->REAL", "REAL"]])
    );
}

#[spec_test(REQ_IC_analyzer_025)]
#[test]
fn apply_when_fold_has_three_inputs_then_written_as_the_calls_it_folds_to() {
    let source = arithmetic_program("LINT", "d : DINT; e : DINT; l : LINT;", "ADD(d, e, l)");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["DINT->LINT", "LINT"], &["DINT", "DINT"]])
    );
}

#[spec_test(REQ_IC_analyzer_026)]
#[test]
fn apply_when_arithmetic_operand_is_arithmetic_then_its_result_is_converted() {
    let source = arithmetic_program("REAL", "i : INT; j : INT; r : REAL;", "(i + j) * r");
    assert_eq!(
        arithmetic_operands(&source),
        operands(&[&["INT->REAL", "REAL"], &["INT", "INT"]])
    );
}

#[spec_test(REQ_IC_analyzer_030)]
#[test]
fn apply_when_variable_assigned_to_wider_target_then_converted_to_target_type() {
    let source = arithmetic_program("LINT", "d : DINT;", "d");
    assert_eq!(assigned_values(&source), vec!["DINT->LINT"]);
}

#[test]
fn apply_when_integer_assigned_to_real_target_then_converted_to_real() {
    let source = arithmetic_program("REAL", "i : INT;", "i");
    assert_eq!(assigned_values(&source), vec!["INT->REAL"]);
}

#[spec_test(REQ_IC_analyzer_031)]
#[test]
fn apply_when_arithmetic_assigned_to_wider_target_then_result_converted() {
    let source = arithmetic_program("LINT", "d : DINT; e : DINT;", "d + e");
    assert_eq!(assigned_values(&source), vec!["DINT->LINT"]);
}

#[test]
fn apply_when_parenthesized_variable_assigned_to_wider_target_then_converted() {
    let source = arithmetic_program("LINT", "d : DINT;", "(d)");
    assert_eq!(assigned_values(&source), vec!["DINT->LINT"]);
}

#[test]
fn apply_when_function_form_assigned_to_wider_target_then_result_converted() {
    let source = arithmetic_program("LINT", "d : DINT; e : DINT;", "ADD(d, e)");
    assert_eq!(assigned_values(&source), vec!["DINT->LINT"]);
}

#[spec_test(REQ_IC_analyzer_032)]
#[test]
fn apply_when_value_shares_target_width_then_unchanged() {
    let source = arithmetic_program("INT", "s : SINT;", "s");
    assert_eq!(assigned_values(&source), vec!["SINT"]);
}

#[spec_test(REQ_IC_analyzer_037)]
#[rstest]
#[case::negation("LINT", "d : DINT;", "-d", "DINT->LINT")]
#[case::abs("LINT", "d : DINT;", "ABS(d)", "DINT->LINT")]
#[case::not("LWORD", "w : DWORD;", "NOT w", "DWORD->LWORD")]
#[case::not_form("LWORD", "w : DWORD;", "NOT(w)", "DWORD->LWORD")]
#[case::shift("LWORD", "w : DWORD;", "SHL(w, 1)", "DWORD->LWORD")]
#[case::move_form("LINT", "d : DINT;", "MOVE(d)", "DINT->LINT")]
#[case::real_function("LREAL", "r : REAL;", "SQRT(r)", "REAL->LREAL")]
fn apply_when_operation_on_one_value_assigned_to_wider_target_then_result_converted(
    #[case] target: &str,
    #[case] vars: &str,
    #[case] value: &str,
    #[case] expected: &str,
) {
    let source = arithmetic_program(target, vars, value);
    assert_eq!(assigned_values(&source), vec![expected]);
}

#[spec_test(REQ_IC_analyzer_034)]
#[test]
fn apply_when_array_element_and_structure_field_targets_then_converted_to_their_types() {
    let source = "TYPE Point : STRUCT x : LINT; END_STRUCT; END_TYPE
        PROGRAM main VAR a : ARRAY[1..2] OF LINT; p : Point; d : DINT; END_VAR
        a[1] := d; p.x := d; END_PROGRAM";
    assert_eq!(assigned_values(source), vec!["DINT->LINT", "DINT->LINT"]);
}

#[spec_test(REQ_IC_analyzer_035)]
#[test]
fn apply_when_function_result_assigned_then_converted_to_result_type() {
    let source = "FUNCTION widen : LINT VAR_INPUT d : DINT; END_VAR widen := d; END_FUNCTION
        PROGRAM main VAR l : LINT; END_VAR l := widen(1); END_PROGRAM";
    assert_eq!(assigned_values(source), vec!["DINT->LINT", "LINT"]);
}

#[spec_test(REQ_IC_analyzer_036)]
#[test]
fn apply_when_subrange_target_then_converted_to_base_type() {
    let source = "TYPE Small : DINT (0..10); END_TYPE
        PROGRAM main VAR s : Small; l : LINT; END_VAR s := l; END_PROGRAM";
    assert_eq!(assigned_values(source), vec!["LINT->DINT"]);
}

/// The inputs of every call to `name` in `source`, which must be free of
/// diagnostics, each as [`describe`] shows it.
fn call_arguments(source: &str, name: &str) -> Vec<String> {
    struct Arguments<'a> {
        types: &'a TypeEnvironment,
        name: &'a str,
        arguments: Vec<String>,
    }
    impl Visitor<Infallible> for Arguments<'_> {
        type Value = ();
        fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
            if node.name.original().eq_ignore_ascii_case(self.name) {
                let inputs = node.param_assignment.iter().filter_map(|p| p.input_expr());
                self.arguments
                    .extend(inputs.map(|e| describe(self.types, e)));
            }
            node.recurse_visit(self)
        }
    }
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = Arguments {
        types: context.types(),
        name,
        arguments: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.arguments
}

/// A function `f` whose one input is of type `param`, called as `f(<args>)`
/// from a program declaring `VAR <vars> END_VAR`.
fn call_program(param: &str, vars: &str, args: &str) -> String {
    format!(
        "FUNCTION f : DINT VAR_INPUT x : {param}; END_VAR f := 0; END_FUNCTION
        PROGRAM main VAR r : DINT; {vars} END_VAR r := f({args}); END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_040)]
#[test]
fn apply_when_argument_narrower_than_parameter_then_converted_to_parameter_type() {
    let source = call_program("LINT", "d : DINT;", "d");
    assert_eq!(call_arguments(&source, "f"), vec!["DINT->LINT"]);
}

#[test]
fn apply_when_argument_is_negation_then_converted_to_parameter_type() {
    let source = call_program("LINT", "d : DINT;", "-d");
    assert_eq!(call_arguments(&source, "f"), vec!["DINT->LINT"]);
}

#[spec_test(REQ_IC_analyzer_041)]
#[test]
fn apply_when_argument_shares_parameter_width_then_unchanged() {
    let source = call_program("INT", "s : SINT;", "s");
    assert_eq!(call_arguments(&source, "f"), vec!["SINT"]);
}

#[spec_test(REQ_IC_analyzer_042)]
#[test]
fn apply_when_literal_argument_of_parameter_width_then_takes_parameter_type() {
    let source = call_program("INT", "", "1");
    assert_eq!(call_arguments(&source, "f"), vec!["INT"]);
}

#[spec_test(REQ_IC_analyzer_042)]
#[test]
fn apply_when_literal_argument_to_wider_parameter_then_takes_parameter_type() {
    let source = call_program("LINT", "", "5000000000");
    assert_eq!(call_arguments(&source, "f"), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_042)]
#[test]
fn apply_when_real_literal_argument_to_lreal_parameter_then_takes_parameter_type() {
    let source = call_program("LREAL", "", "0.1");
    assert_eq!(call_arguments(&source, "f"), vec!["LREAL"]);
}

#[spec_test(REQ_IC_analyzer_042)]
#[test]
fn apply_when_integer_literal_argument_to_real_parameter_then_takes_parameter_type() {
    let source = call_program("LREAL", "", "1");
    assert_eq!(call_arguments(&source, "f"), vec!["LREAL"]);
}

#[spec_test(REQ_IC_analyzer_043)]
#[test]
fn apply_when_argument_to_parameter_of_alias_then_not_converted() {
    let source = "TYPE Big : LINT; Precise : LREAL; END_TYPE
        FUNCTION f : DINT VAR_INPUT x : Big; END_VAR f := 0; END_FUNCTION
        FUNCTION g : DINT VAR_INPUT x : Precise; END_VAR g := 0; END_FUNCTION
        PROGRAM main VAR r : DINT; b : Big; p : Precise; END_VAR
        r := f(b); r := g(p); END_PROGRAM";
    assert_eq!(call_arguments(source, "f"), vec!["LINT"]);
    assert_eq!(call_arguments(source, "g"), vec!["LREAL"]);
}

#[spec_test(REQ_IC_analyzer_043)]
#[test]
fn apply_when_argument_to_parameter_of_subrange_then_not_converted() {
    let source = "TYPE Small : LINT (0..100); END_TYPE
        FUNCTION f : DINT VAR_INPUT x : Small; END_VAR f := 0; END_FUNCTION
        PROGRAM main VAR r : DINT; s : Small; END_VAR r := f(s); END_PROGRAM";
    assert_eq!(call_arguments(source, "f"), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_044)]
#[test]
fn apply_when_named_argument_then_converted_as_positional() {
    let source = call_program("LINT", "d : DINT;", "x := d");
    assert_eq!(call_arguments(&source, "f"), vec!["DINT->LINT"]);
}

#[spec_test(REQ_IC_analyzer_045)]
#[test]
fn apply_when_standard_function_or_in_out_parameter_then_arguments_unchanged() {
    let source = "FUNCTION g : DINT VAR_IN_OUT x : DINT; END_VAR g := x; END_FUNCTION
        PROGRAM main VAR r : LINT; d : DINT; s : DINT; END_VAR
        r := ABS(d); s := g(d); END_PROGRAM";
    assert_eq!(call_arguments(source, "ABS"), vec!["DINT"]);
    assert_eq!(call_arguments(source, "g"), vec!["DINT"]);
}

/// The type of every literal in `source`, which must be free of
/// diagnostics, in source order.
fn literal_types(source: &str) -> Vec<String> {
    literal_types_with(source, &CompilerOptions::default())
}

/// [`literal_types`] under `options`.
pub(super) fn literal_types_with(source: &str, options: &CompilerOptions) -> Vec<String> {
    struct Literals<'a> {
        types: &'a TypeEnvironment,
        literals: Vec<String>,
    }
    impl Visitor<Infallible> for Literals<'_> {
        type Value = ();
        fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
            if let ExprKind::Const(_) = node.kind {
                self.literals.push(describe(self.types, node));
            }
            node.recurse_visit(self)
        }
    }
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = Literals {
        types: context.types(),
        literals: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.literals
}

#[spec_test(REQ_IC_analyzer_050)]
#[test]
fn apply_when_literal_assigned_then_takes_target_type() {
    let source = arithmetic_program("LINT", "", "1");
    assert_eq!(literal_types(&source), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_051)]
#[test]
fn apply_when_literal_under_negation_and_parentheses_then_takes_context_type() {
    let source = arithmetic_program("LINT", "", "-(1)");
    assert_eq!(literal_types(&source), vec!["LINT"]);
}

#[test]
fn apply_when_literal_operand_of_typed_arithmetic_then_takes_operation_type() {
    let source = arithmetic_program("LINT", "d : DINT;", "d + -1");
    assert_eq!(literal_types(&source), vec!["DINT"]);
}

#[spec_test(REQ_IC_analyzer_053)]
#[test]
fn apply_when_for_loop_then_bounds_and_step_take_control_type() {
    let source = "PROGRAM main VAR i : INT; END_VAR
        FOR i := 1 TO 10 BY 2 DO i := i; END_FOR; END_PROGRAM";
    assert_eq!(literal_types(source), vec!["INT", "INT", "INT"]);
}

#[spec_test(REQ_IC_analyzer_054)]
#[test]
fn apply_when_function_block_input_then_takes_declared_type_of_its_field() {
    let source = "TYPE R : LINT (0..5000000000); END_TYPE
        FUNCTION_BLOCK Acc VAR_INPUT n : LINT; r : R; END_VAR END_FUNCTION_BLOCK
        PROGRAM main VAR a : Acc; c : CTU; END_VAR
        a(n := 1, r := 4000000000); a(2, 3); c(CU := TRUE, PV := 5); END_PROGRAM";
    assert_eq!(
        literal_types(source),
        vec!["LINT", "LINT", "LINT", "LINT", "BOOL", "INT"]
    );
}

#[spec_test(REQ_IC_analyzer_055)]
#[test]
fn apply_when_dereferenced_target_then_literal_takes_referenced_type() {
    let source = "PROGRAM main VAR l : LINT; r : REF_TO LINT; END_VAR
        r := REF(l); r^ := 5; END_PROGRAM";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    assert_eq!(literal_types_with(source, &options), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_056)]
#[test]
fn apply_when_subrange_target_then_literal_takes_base_type() {
    let source = "TYPE Small : LINT (0..10); END_TYPE
        PROGRAM main VAR s : Small; END_VAR s := 3; END_PROGRAM";
    assert_eq!(literal_types(source), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_057)]
#[test]
fn apply_when_literal_input_of_function_of_inputs_of_one_type_then_takes_its_type() {
    let source = arithmetic_program("LINT", "d : DINT;", "MAX(d, 5)");
    assert_eq!(literal_types(&source), vec!["DINT"]);
    assert_eq!(assigned_values(&source), vec!["DINT->LINT"]);
}

#[spec_test(REQ_IC_analyzer_058)]
#[test]
fn apply_when_selector_of_mux_or_sel_then_takes_default_type() {
    let source = arithmetic_program("LINT", "a : LINT; b : LINT;", "MUX(1, a, b)");
    assert_eq!(literal_types(&source), vec!["DINT"]);
    let source = arithmetic_program("LINT", "a : LINT;", "SEL(TRUE, a, 7)");
    assert_eq!(literal_types(&source), vec!["BOOL", "LINT"]);
}

#[spec_test(REQ_IC_analyzer_059)]
#[test]
fn apply_when_shift_count_then_takes_width_of_shifted_value() {
    let source = arithmetic_program("LWORD", "w : LWORD;", "SHL(w, 2)");
    assert_eq!(literal_types(&source), vec!["LINT"]);
    let source = arithmetic_program("DWORD", "w : DWORD;", "SHL(w, 2)");
    assert_eq!(literal_types(&source), vec!["DINT"]);
}

#[spec_test(REQ_IC_analyzer_067)]
#[rstest]
#[case::shift("SHL(w, 1)")]
#[case::rotate("ROL(w, 1)")]
fn apply_when_literal_input_of_operation_on_one_value_then_takes_its_type(#[case] value: &str) {
    let source = arithmetic_program("LWORD", "w : DWORD;", value);
    assert_eq!(literal_types(&source), vec!["DINT"]);
}

#[spec_test(REQ_IC_analyzer_060)]
#[test]
fn apply_when_string_function_position_then_takes_default_type() {
    let source = arithmetic_program("STRING", "s : STRING;", "LEFT(s, 3)");
    assert_eq!(literal_types(&source), vec!["DINT"]);
}

#[test]
fn apply_when_string_function_nested_in_string_function_then_its_positions_typed() {
    let source = arithmetic_program("DINT", "s : STRING;", "LEN(MID(s, 2, 3))");
    assert_eq!(literal_types(&source), vec!["DINT", "DINT"]);
}

#[spec_test(REQ_IC_analyzer_061)]
#[test]
fn apply_when_conversion_input_then_takes_source_type() {
    let source = arithmetic_program("REAL", "", "INT_TO_REAL(5)");
    assert_eq!(literal_types(&source), vec!["INT"]);
}

#[spec_test(REQ_IC_analyzer_062)]
#[test]
fn apply_when_comparison_of_two_literals_then_both_take_left_default_type() {
    let source = arithmetic_program("BOOL", "", "1 < 2");
    assert_eq!(literal_types(&source), vec!["DINT", "DINT"]);
}

#[spec_test(REQ_IC_analyzer_063)]
#[test]
fn apply_when_method_argument_then_takes_declared_type_of_its_parameter() {
    let source = "TYPE R : LINT (0..5000000000); END_TYPE
        FUNCTION_BLOCK Acc
        METHOD add : LINT VAR_INPUT n : LINT; END_VAR add := n; END_METHOD
        METHOD addr : LINT VAR_INPUT n : R; END_VAR addr := n; END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main VAR a : Acc; r : LINT; END_VAR
        r := a.add(1); r := a.addr(4000000000); END_PROGRAM";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_eq!(literal_types_with(source, &options), vec!["LINT", "LINT"]);
}

#[spec_test(REQ_IC_analyzer_064)]
#[test]
fn apply_when_subscript_then_takes_default_type() {
    let source = "PROGRAM main VAR a : ARRAY[1..2, 1..2] OF LINT; i : DINT; x : LINT; END_VAR
        x := a[1, i]; END_PROGRAM";
    assert_eq!(literal_types(source), vec!["DINT"]);
}

#[spec_test(REQ_IC_analyzer_065)]
#[test]
fn apply_when_case_selector_then_literals_take_its_type() {
    let source = "PROGRAM main VAR d : DINT; x : DINT; END_VAR
        CASE MAX(d, 1) OF 1: x := 2; END_CASE; END_PROGRAM";
    assert_eq!(literal_types(source), vec!["DINT", "DINT"]);
}

/// The first literal is the function's `f := 0`; the second is the
/// argument's, which the negation passes its parameter's type to.
#[test]
fn apply_when_literal_argument_under_negation_then_takes_parameter_type() {
    let source = call_program("LINT", "", "-1");
    assert_eq!(literal_types(&source), vec!["DINT", "LINT"]);
}

#[test]
fn apply_when_method_call_spelled_in_another_case_then_argument_takes_parameter_type() {
    let source = "FUNCTION_BLOCK Acc
        METHOD Add : LINT VAR_INPUT N : LINT; END_VAR Add := N; END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main VAR a : Acc; r : LINT; END_VAR r := a.ADD(n := 1); END_PROGRAM";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_eq!(literal_types_with(source, &options), vec!["LINT"]);
}

#[spec_test(REQ_IC_analyzer_066)]
#[test]
fn apply_when_typed_literal_in_context_of_another_type_then_converted_to_it() {
    let source = arithmetic_program("LINT", "", "UDINT#4000000000");
    assert_eq!(assigned_values(&source), vec!["UDINT->LINT"]);
}

#[test]
fn apply_when_typed_literal_in_context_of_its_own_type_then_unchanged() {
    let source = arithmetic_program("LINT", "", "LINT#5");
    assert_eq!(assigned_values(&source), vec!["LINT"]);
}

/// How the type of each literal in `source` was recorded: `inferred` or
/// `stated`.
fn literal_type_origins(source: &str) -> Vec<&'static str> {
    struct Origins(Vec<&'static str>);
    impl Visitor<Infallible> for Origins {
        type Value = ();
        fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
            if let ExprKind::Const(_) = node.kind {
                self.0.push(match node.expr_type {
                    Some(ExprType::Inferred(_)) => "inferred",
                    Some(ExprType::Concrete(_)) => "stated",
                    Some(ExprType::Literal(_) | ExprType::Null) | None => "none",
                });
            }
            node.recurse_visit(self)
        }
    }
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, _context) = analyze(&[&library], &options).unwrap();
    let mut origins = Origins(vec![]);
    let Ok(()) = origins.walk(&library);
    origins.0
}

#[spec_test(REQ_IC_analyzer_068)]
#[test]
fn apply_when_untyped_literal_typed_then_type_recorded_as_inferred() {
    let source = "PROGRAM main VAR l : LINT; END_VAR l := 1; l := LINT#1; END_PROGRAM";
    assert_eq!(literal_type_origins(source), vec!["inferred", "stated"]);
}

mod bitwise;
mod call_result;
mod inputs_of_one_type;
mod integer_result;
mod subrange;
mod temporal_assignment;
