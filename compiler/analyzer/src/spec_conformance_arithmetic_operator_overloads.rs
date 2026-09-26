//! Spec conformance tests for arithmetic operator overloads (analyzer-owned
//! requirements).
//!
//! Each test is annotated with `#[spec_test(REQ_AO_analyzer_NNN)]`, which
//! adds `#[test]` and references a build-script-generated constant so the
//! test fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance`
//! asserts every analyzer-owned requirement has a test.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use std::convert::Infallible;

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{FileId, Id};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Assignment, Expr, ExprKind, Operator};
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, FoldFailure, Overload,
};
use crate::intermediates::operator_function_form::operator_function_form;
use crate::intermediates::stdlib_function::get_all_stdlib_functions;
use crate::intermediates::stdlib_time_function::long_form;
use crate::stages::{analyze, resolve_types};

/// The numeric types of `ANY_NUM`.
const ANY_NUM: [&str; 10] = [
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL",
];

/// Resolves `op` on operands of the named types under default options.
fn resolve(op: Operator, left: &str, right: &str) -> Option<Overload> {
    resolve_arithmetic_overload(
        &op,
        Some(&TypeName::from(left)),
        Some(&TypeName::from(right)),
        &CompilerOptions::default(),
    )
}

fn numeric(result: &str) -> Option<Overload> {
    Some(Overload::Numeric {
        result: TypeName::from(result),
    })
}

fn typed(name: &'static str, result: &str) -> Option<Overload> {
    Some(Overload::Typed {
        name,
        result: TypeName::from(result),
    })
}

/// REQ-AO-analyzer-001: two operands of the same numeric type resolve to the
/// numeric overload with that type.
#[spec_test(REQ_AO_analyzer_001)]
#[rstest]
fn analyzer_spec_req_ao_001_same_numeric_type_resolves_to_that_type(
    #[values(Operator::Add, Operator::Sub, Operator::Mul, Operator::Div)] op: Operator,
) {
    for operand in ANY_NUM {
        assert_eq!(
            resolve(op.clone(), operand, operand),
            numeric(operand),
            "{op:?} on {operand}"
        );
    }
    for operand in [
        "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT",
    ] {
        assert_eq!(
            resolve(Operator::Mod, operand, operand),
            numeric(operand),
            "MOD on {operand}"
        );
    }
}

/// REQ-AO-analyzer-002: where one operand widens to the other, the result is
/// the wider type, whichever side it is on.
#[spec_test(REQ_AO_analyzer_002)]
#[rstest]
#[case("INT", "DINT", "DINT")]
#[case("DINT", "INT", "DINT")]
#[case("USINT", "INT", "INT")]
#[case("UDINT", "LINT", "LINT")]
#[case("INT", "REAL", "REAL")]
#[case("DINT", "LREAL", "LREAL")]
#[case("REAL", "LREAL", "LREAL")]
fn analyzer_spec_req_ao_002_widening_pair_resolves_to_wider_type(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result: &str,
) {
    assert_eq!(resolve(Operator::Add, left, right), numeric(result));
}

/// REQ-AO-analyzer-003: a bare integer literal takes the other operand's
/// type, including `REAL` and `LREAL`, on either side.
#[spec_test(REQ_AO_analyzer_003)]
#[rstest]
#[case("DINT", "ANY_INT", "DINT")]
#[case("ANY_INT", "DINT", "DINT")]
#[case("REAL", "ANY_INT", "REAL")]
#[case("ANY_INT", "LREAL", "LREAL")]
#[case("USINT", "ANY_INT", "USINT")]
fn analyzer_spec_req_ao_003_integer_literal_takes_other_operand_type(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result: &str,
) {
    assert_eq!(resolve(Operator::Mul, left, right), numeric(result));
}

/// REQ-AO-analyzer-004: numeric operands where neither widens to the other
/// do not resolve.
#[spec_test(REQ_AO_analyzer_004)]
#[rstest]
#[case("DINT", "REAL")]
#[case("REAL", "DINT")]
#[case("DINT", "UDINT")]
#[case("SINT", "USINT")]
#[case("DINT", "ANY_REAL")]
fn analyzer_spec_req_ao_004_non_widening_pair_does_not_resolve(
    #[case] left: &str,
    #[case] right: &str,
) {
    assert_eq!(resolve(Operator::Add, left, right), None);
}

/// Every Table 30 row: the operator, a pair of short operand types, the typed
/// name, and the result type. `ANY_NUM` is exercised with `DINT`.
const TABLE_30: [(Operator, &str, &str, &str, &str); 11] = [
    (Operator::Add, "TIME", "TIME", "ADD_TIME", "TIME"),
    (
        Operator::Add,
        "TIME_OF_DAY",
        "TIME",
        "ADD_TOD_TIME",
        "TIME_OF_DAY",
    ),
    (
        Operator::Add,
        "DATE_AND_TIME",
        "TIME",
        "ADD_DT_TIME",
        "DATE_AND_TIME",
    ),
    (Operator::Sub, "TIME", "TIME", "SUB_TIME", "TIME"),
    (Operator::Sub, "DATE", "DATE", "SUB_DATE_DATE", "TIME"),
    (
        Operator::Sub,
        "TIME_OF_DAY",
        "TIME",
        "SUB_TOD_TIME",
        "TIME_OF_DAY",
    ),
    (
        Operator::Sub,
        "TIME_OF_DAY",
        "TIME_OF_DAY",
        "SUB_TOD_TOD",
        "TIME",
    ),
    (
        Operator::Sub,
        "DATE_AND_TIME",
        "TIME",
        "SUB_DT_TIME",
        "DATE_AND_TIME",
    ),
    (
        Operator::Sub,
        "DATE_AND_TIME",
        "DATE_AND_TIME",
        "SUB_DT_DT",
        "TIME",
    ),
    (Operator::Mul, "TIME", "DINT", "MUL_TIME", "TIME"),
    (Operator::Div, "TIME", "DINT", "DIV_TIME", "TIME"),
];

/// Returns the long type of a short temporal type, or the type itself.
fn long(short: &str) -> &str {
    match short {
        "TIME" => "LTIME",
        "DATE" => "LDATE",
        "TIME_OF_DAY" => "LTIME_OF_DAY",
        "DATE_AND_TIME" => "LDATE_AND_TIME",
        other => other,
    }
}

/// REQ-AO-analyzer-005: each Table 30 pair resolves to its typed name with
/// the typed function's return type.
#[spec_test(REQ_AO_analyzer_005)]
fn analyzer_spec_req_ao_005_table_30_pair_resolves_to_typed_name() {
    for (op, left, right, name, result) in TABLE_30 {
        assert_eq!(
            resolve(op.clone(), left, right),
            typed(name, result),
            "{op:?} on {left}, {right}"
        );
    }
}

/// REQ-AO-analyzer-006: each pair of long operand types resolves to the long
/// form, not the short one.
#[spec_test(REQ_AO_analyzer_006)]
fn analyzer_spec_req_ao_006_long_pair_resolves_to_long_form() {
    for (op, left, right, name, result) in TABLE_30 {
        let long_name = long_form(name).unwrap();
        assert_eq!(
            resolve(op.clone(), long(left), long(right)),
            typed(long_name, long(result)),
            "{op:?} on {}, {}",
            long(left),
            long(right)
        );
    }
}

/// REQ-AO-analyzer-007: mixing the two widths of a family resolves to the
/// long form, so `t + lt` and `lt + LTIME#1s` (whose literal is typed `TIME`)
/// are `ADD_LTIME`.
#[spec_test(REQ_AO_analyzer_007)]
#[rstest]
#[case(Operator::Add, "TIME", "LTIME", "ADD_LTIME", "LTIME")]
#[case(Operator::Add, "LTIME", "TIME", "ADD_LTIME", "LTIME")]
#[case(
    Operator::Add,
    "DATE_AND_TIME",
    "LTIME",
    "ADD_LDT_LTIME",
    "LDATE_AND_TIME"
)]
#[case(Operator::Sub, "LDATE", "DATE", "SUB_LDATE_LDATE", "LTIME")]
#[case(Operator::Mul, "LTIME", "ANY_INT", "MUL_LTIME", "LTIME")]
fn analyzer_spec_req_ao_007_mixed_width_pair_resolves_to_long_form(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
    #[case] name: &'static str,
    #[case] result: &str,
) {
    assert_eq!(resolve(op, left, right), typed(name, result));
}

/// REQ-AO-analyzer-008: `TIME` times or divided by any `ANY_NUM` operand
/// resolves to `MUL_TIME` or `DIV_TIME`; a number times `TIME` does not.
#[spec_test(REQ_AO_analyzer_008)]
fn analyzer_spec_req_ao_008_time_scaled_by_number_resolves_only_time_first() {
    for factor in ANY_NUM.iter().chain(&["ANY_INT", "ANY_REAL"]) {
        assert_eq!(
            resolve(Operator::Mul, "TIME", factor),
            typed("MUL_TIME", "TIME"),
            "TIME * {factor}"
        );
        assert_eq!(
            resolve(Operator::Div, "TIME", factor),
            typed("DIV_TIME", "TIME"),
            "TIME / {factor}"
        );
        assert_eq!(
            resolve(Operator::Mul, factor, "TIME"),
            None,
            "{factor} * TIME"
        );
    }
}

/// REQ-AO-analyzer-009: strings, `BOOL`, and temporal pairs with no Table 30
/// row do not resolve.
#[spec_test(REQ_AO_analyzer_009)]
#[rstest]
#[case(Operator::Add, "STRING", "STRING")]
#[case(Operator::Add, "WSTRING", "WSTRING")]
#[case(Operator::Mul, "BOOL", "BOOL")]
#[case(Operator::Add, "TIME", "DATE")]
#[case(Operator::Add, "DATE", "DATE")]
#[case(Operator::Add, "DATE", "TIME")]
#[case(Operator::Mul, "TIME", "TIME")]
#[case(Operator::Div, "TIME", "TIME")]
#[case(Operator::Sub, "TIME", "TIME_OF_DAY")]
#[case(Operator::Mod, "TIME", "TIME")]
fn analyzer_spec_req_ao_009_pair_without_overload_does_not_resolve(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
) {
    assert_eq!(resolve(op, left, right), None);
}

/// Resolves `op` on operands of the named types with
/// `--allow-bit-string-arithmetic` on or off.
fn resolve_with_bit_strings(op: Operator, left: &str, right: &str, on: bool) -> Option<Overload> {
    let options = CompilerOptions {
        allow_bit_string_arithmetic: on,
        ..CompilerOptions::default()
    };
    resolve_arithmetic_overload(
        &op,
        Some(&TypeName::from(left)),
        Some(&TypeName::from(right)),
        &options,
    )
}

/// REQ-AO-analyzer-010: with the flag, a bit string resolves as the unsigned
/// integer of its width: two bit strings give the wider one, and a bit
/// string with a number gives what the widening picks.
#[spec_test(REQ_AO_analyzer_010)]
#[rstest]
#[case("BYTE", "BYTE", Some("BYTE"))]
#[case("BYTE", "WORD", Some("WORD"))]
#[case("LWORD", "DWORD", Some("LWORD"))]
#[case("BYTE", "ANY_INT", Some("BYTE"))]
#[case("ANY_INT", "WORD", Some("WORD"))]
#[case("BYTE", "INT", Some("INT"))]
#[case("BYTE", "REAL", Some("REAL"))]
#[case("DWORD", "LINT", Some("LINT"))]
// Judged as UINT, a WORD neither widens to INT nor is widened to by it.
#[case("WORD", "INT", None)]
#[case("BYTE", "SINT", None)]
// BOOL is never an integer.
#[case("BOOL", "BOOL", None)]
#[case("BOOL", "ANY_INT", None)]
fn analyzer_spec_req_ao_010_bit_string_resolves_as_unsigned_integer_with_flag(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result: Option<&str>,
) {
    for op in [Operator::Add, Operator::Sub, Operator::Mul, Operator::Div] {
        assert_eq!(
            resolve_with_bit_strings(op.clone(), left, right, true),
            result.and_then(numeric),
            "{op:?} on {left}, {right}"
        );
    }
}

/// REQ-AO-analyzer-011: without the flag, a bit-string operand does not
/// resolve.
#[spec_test(REQ_AO_analyzer_011)]
#[rstest]
#[case("BYTE", "BYTE")]
#[case("BYTE", "ANY_INT")]
#[case("WORD", "UINT")]
#[case("DINT", "DWORD")]
#[case("LWORD", "LWORD")]
fn analyzer_spec_req_ao_011_bit_string_does_not_resolve_without_flag(
    #[case] left: &str,
    #[case] right: &str,
) {
    for op in [Operator::Add, Operator::Sub, Operator::Mul, Operator::Div] {
        assert_eq!(
            resolve_with_bit_strings(op.clone(), left, right, false),
            None,
            "{op:?} on {left}, {right}"
        );
    }
}

/// REQ-AO-analyzer-015: the flag does not apply to `MOD`, so `b MOD 2` on a
/// bit string does not resolve, as `MOD(b, 2)` is rejected by its signature.
#[spec_test(REQ_AO_analyzer_015)]
#[rstest]
#[case("BYTE", "ANY_INT")]
#[case("WORD", "WORD")]
#[case("DWORD", "UDINT")]
fn analyzer_spec_req_ao_015_mod_on_bit_string_does_not_resolve_with_flag(
    #[case] left: &str,
    #[case] right: &str,
) {
    assert_eq!(
        resolve_with_bit_strings(Operator::Mod, left, right, true),
        None
    );
}

/// REQ-AO-analyzer-012: an operand with no type, or a type the predicate
/// cannot judge, resolves as unchecked with the left operand's type, which
/// is not the numeric overload.
#[spec_test(REQ_AO_analyzer_012)]
fn analyzer_spec_req_ao_012_unjudged_operand_resolves_as_unchecked() {
    let options = CompilerOptions::default();
    let subrange = TypeName::from("MY_RANGE");
    let dint = TypeName::from("DINT");
    let int_literal = TypeName::from("ANY_INT");

    let judged = resolve_arithmetic_overload(
        &Operator::Add,
        Some(&subrange),
        Some(&int_literal),
        &options,
    );
    assert_eq!(
        judged,
        Some(Overload::Unchecked {
            result: Some(subrange.clone())
        })
    );
    assert!(!matches!(judged, Some(Overload::Numeric { .. })));

    assert_eq!(
        resolve_arithmetic_overload(&Operator::Add, Some(&dint), None, &options),
        Some(Overload::Unchecked {
            result: Some(dint.clone())
        })
    );
    assert_eq!(
        resolve_arithmetic_overload(&Operator::Add, None, Some(&dint), &options),
        Some(Overload::Unchecked { result: None })
    );
}

/// REQ-AO-analyzer-013: an extensible call folds from the left, so
/// `ADD(t1, t2, t3)` resolves and `ADD(t1, t2, r)` fails at its second step.
#[spec_test(REQ_AO_analyzer_013)]
fn analyzer_spec_req_ao_013_extensible_call_resolves_by_folding_left() {
    let options = CompilerOptions::default();
    let time = TypeName::from("TIME");
    let real = TypeName::from("REAL");

    assert_eq!(
        resolve_arithmetic_fold(
            &Operator::Add,
            &[Some(&time), Some(&time), Some(&time)],
            &options
        ),
        Ok(Overload::Typed {
            name: "ADD_TIME",
            result: time.clone()
        })
    );
    assert_eq!(
        resolve_arithmetic_fold(
            &Operator::Add,
            &[Some(&time), Some(&time), Some(&real)],
            &options
        ),
        Err(FoldFailure {
            left: time.clone(),
            right: real.clone()
        })
    );
}

/// REQ-AO-analyzer-014: every typed name in the operator-form table, in both
/// widths, is a registered function signature with two inputs.
#[spec_test(REQ_AO_analyzer_014)]
fn analyzer_spec_req_ao_014_every_typed_name_is_a_registered_two_input_function() {
    let registered = get_all_stdlib_functions();
    let mut checked = 0;
    for function in ["ADD", "SUB", "MUL", "DIV", "MOD"] {
        let form = operator_function_form(function).unwrap();
        for short in form.typed_overloads() {
            for name in [*short, long_form(short).unwrap()] {
                let signature = registered.iter().find(|sig| sig.name == Id::from(name));
                assert!(signature.is_some(), "{name} is not registered");
                let signature = signature.unwrap();
                assert_eq!(signature.parameters.len(), 2, "{name}");
                assert!(signature.parameters.iter().all(|p| p.is_input), "{name}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 22);
}

// ---------------------------------------------------------------------------
// Type resolution and diagnostics, through the analysis pipeline.
// ---------------------------------------------------------------------------

/// Options with the long temporal types, which the long-form cases declare.
fn edition_3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

/// A program declaring `vars` and assigning `expr` to `result : result_type`.
fn program(vars: &str, result_type: &str, expr: &str) -> String {
    format!(
        "PROGRAM main
VAR
    result : {result_type};
    {vars}
END_VAR
    result := {expr};
END_PROGRAM"
    )
}

/// Resolves types in `source` and returns the value of every assignment.
fn assigned_values(source: &str, options: &CompilerOptions) -> Vec<Expr> {
    struct Values(Vec<Expr>);
    impl Visitor<Infallible> for Values {
        type Value = ();
        fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
            self.0.push(node.value.clone());
            node.recurse_visit(self)
        }
    }
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (library, _context) = resolve_types(&[&library], options).unwrap();
    let mut values = Values(vec![]);
    let _ = values.walk(&library);
    values.0
}

/// The resolved type of the single assignment's value in `source`.
fn assigned_type(source: &str, options: &CompilerOptions) -> Option<TypeName> {
    let values = assigned_values(source, options);
    assert_eq!(values.len(), 1);
    values[0].resolved_type.clone()
}

/// Analyzes `source` and returns its diagnostics.
fn diagnostics(source: &str, options: &CompilerOptions) -> Vec<Diagnostic> {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (_library, context) = analyze(&[&library], options).unwrap();
    context.diagnostics().to_vec()
}

fn p4049() -> String {
    Problem::OperatorOperandTypeMismatch.code().to_string()
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.code.clone()).collect()
}

/// REQ-AO-analyzer-020: an arithmetic expression has the result type of its
/// overload.
#[spec_test(REQ_AO_analyzer_020)]
#[rstest]
#[case("i : INT; r : REAL;", "REAL", "i + r", "REAL")]
#[case("i : INT; d : DINT;", "DINT", "i + d", "DINT")]
#[case("t1 : TIME; t2 : TIME;", "TIME", "t1 + t2", "TIME")]
#[case("d1 : DATE; d2 : DATE;", "TIME", "d1 - d2", "TIME")]
#[case("t : TIME; lt : LTIME;", "LTIME", "t + lt", "LTIME")]
#[case("t : TIME; r : REAL;", "TIME", "t * r", "TIME")]
fn analyzer_spec_req_ao_020_binary_expression_has_overload_result_type(
    #[case] vars: &str,
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        assigned_type(&program(vars, result_type, expr), &edition_3()),
        Some(TypeName::from(expected)),
        "{expr}"
    );
}

/// REQ-AO-analyzer-021: a call to an overloaded name has the result type of
/// its overload, folded from the left.
#[spec_test(REQ_AO_analyzer_021)]
#[rstest]
#[case("d1 : DATE; d2 : DATE;", "TIME", "SUB(d1, d2)", "TIME")]
#[case("t1 : TIME; t2 : TIME; t3 : TIME;", "TIME", "ADD(t1, t2, t3)", "TIME")]
#[case("t : TIME;", "TIME", "MUL(t, 2)", "TIME")]
#[case("i : INT; d : DINT;", "DINT", "ADD(i, d)", "DINT")]
#[case("i : INT; r : REAL; lr : LREAL;", "LREAL", "ADD(i, r, lr)", "LREAL")]
fn analyzer_spec_req_ao_021_overloaded_call_has_overload_result_type(
    #[case] vars: &str,
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        assigned_type(&program(vars, result_type, expr), &edition_3()),
        Some(TypeName::from(expected)),
        "{expr}"
    );
}

/// REQ-AO-analyzer-022: an expression that is unchecked or does not resolve
/// keeps the left operand's type.
#[spec_test(REQ_AO_analyzer_022)]
#[rstest]
#[case("s1 : STRING; s2 : STRING;", "STRING", "s1 + s2")]
#[case("d : DINT; r : REAL;", "DINT", "d + r")]
#[case("b : BOOL;", "BOOL", "b * b")]
fn analyzer_spec_req_ao_022_unresolved_expression_keeps_left_type(
    #[case] vars: &str,
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    let values = assigned_values(&program(vars, result_type, expr), &edition_3());
    let ExprKind::BinaryOp(binary) = &values[0].kind else {
        panic_free_fail(expr);
        return;
    };
    assert_eq!(values[0].resolved_type, binary.left.resolved_type, "{expr}");
}

/// Fails the enclosing test for an expression of the wrong shape.
fn panic_free_fail(expr: &str) {
    assert!(false, "{expr} is not a binary expression");
}

/// REQ-AO-analyzer-023: resolution does not rewrite the tree: an operator
/// stays a binary expression and a call stays a call to the name written.
#[spec_test(REQ_AO_analyzer_023)]
#[test]
fn analyzer_spec_req_ao_023_resolved_arithmetic_keeps_its_shape() {
    let source = "PROGRAM main
VAR
    t1 : TIME;
    t2 : TIME;
    d1 : DATE;
    d2 : DATE;
END_VAR
    t1 := t1 + t2;
    t1 := ADD(t1, t2);
    t1 := d1 - d2;
END_PROGRAM";
    let values = assigned_values(source, &CompilerOptions::default());
    assert!(matches!(values[0].kind, ExprKind::BinaryOp(_)));
    assert!(matches!(&values[1].kind, ExprKind::Function(f) if f.name == Id::from("ADD")));
    assert!(matches!(values[2].kind, ExprKind::BinaryOp(_)));
}

/// REQ-AO-analyzer-024: literal arithmetic is still folded to a constant.
#[spec_test(REQ_AO_analyzer_024)]
#[rstest]
#[case("DINT", "2 + 3")]
#[case("REAL", "1 + 2.5")]
#[case("LINT", "1000 * 1000")]
fn analyzer_spec_req_ao_024_literal_arithmetic_is_folded(
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    let values = assigned_values(&program("", result_type, expr), &CompilerOptions::default());
    assert!(
        matches!(values[0].kind, ExprKind::Const(_)),
        "{expr}: {:?}",
        values[0].kind
    );
}

/// REQ-AO-analyzer-030: an operator no overload applies to is P4049 naming
/// the operator and both operand types.
#[spec_test(REQ_AO_analyzer_030)]
#[test]
fn analyzer_spec_req_ao_030_unresolved_operator_names_operator_and_operands() {
    let found = diagnostics(
        &program("d : DINT; r : REAL;", "DINT", "d * r"),
        &CompilerOptions::default(),
    );
    assert_eq!(codes(&found), vec![p4049()]);
    let described = &found[0].described;
    for context in ["operator=*", "left=DINT", "right=REAL"] {
        assert!(described.contains(&context.to_owned()), "{described:?}");
    }
}

/// REQ-AO-analyzer-031: every arithmetic operator is checked.
#[spec_test(REQ_AO_analyzer_031)]
#[rstest]
#[case("r : REAL;", "REAL", "r MOD 2.0")]
#[case("t1 : TIME; t2 : TIME;", "TIME", "t1 * t2")]
#[case("s1 : STRING; s2 : STRING;", "STRING", "s1 + s2")]
#[case("x : BOOL;", "BOOL", "x * x")]
#[case("d : DINT; r : REAL;", "DINT", "d - r")]
#[case("t1 : TIME; t2 : TIME;", "TIME", "t1 / t2")]
fn analyzer_spec_req_ao_031_every_arithmetic_operator_is_checked(
    #[case] vars: &str,
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    let found = codes(&diagnostics(
        &program(vars, result_type, expr),
        &CompilerOptions::default(),
    ));
    assert!(found.contains(&p4049()), "{expr}: {found:?}");
}

/// REQ-AO-analyzer-032: a call to an overloaded name that does not resolve is
/// P4049 naming the function and the failing step's operand types, not P4026.
#[spec_test(REQ_AO_analyzer_032)]
#[rstest]
#[case("MUL(t, t)", "MUL", "TIME")]
#[case("ADD(t, t, r)", "ADD", "REAL")]
fn analyzer_spec_req_ao_032_unresolved_call_is_p4049_not_p4026(
    #[case] expr: &str,
    #[case] function: &str,
    #[case] right: &str,
) {
    let found = diagnostics(
        &program("t : TIME; r : REAL;", "TIME", expr),
        &CompilerOptions::default(),
    );
    assert_eq!(codes(&found), vec![p4049()], "{expr}");
    let described = &found[0].described;
    for context in [
        format!("operator={function}"),
        "left=TIME".to_owned(),
        format!("right={right}"),
    ] {
        assert!(described.contains(&context), "{expr}: {described:?}");
    }
}

/// REQ-AO-analyzer-033: an operator expression that resolves is clean.
#[spec_test(REQ_AO_analyzer_033)]
#[rstest]
#[case("t1 : TIME; t2 : TIME;", "TIME", "t1 + t2")]
#[case("clock : TIME_OF_DAY; t : TIME;", "TIME_OF_DAY", "clock + t")]
#[case("d1 : DATE; d2 : DATE;", "TIME", "d1 - d2")]
#[case("t : TIME; lt : LTIME;", "LTIME", "t + lt")]
#[case("i : INT; d : DINT;", "DINT", "i + d")]
#[case("t : TIME;", "TIME", "t * 2")]
fn analyzer_spec_req_ao_033_resolved_operator_is_clean(
    #[case] vars: &str,
    #[case] result_type: &str,
    #[case] expr: &str,
) {
    let found = codes(&diagnostics(
        &program(vars, result_type, expr),
        &edition_3(),
    ));
    assert!(found.is_empty(), "{expr}: {found:?}");
}

/// REQ-AO-analyzer-034: a function call on each Table 30 pair, at both
/// widths, is clean.
#[spec_test(REQ_AO_analyzer_034)]
#[test]
fn analyzer_spec_req_ao_034_call_on_table_30_pair_is_clean() {
    for (op, left, right, _, result) in TABLE_30 {
        let function = match op {
            Operator::Add => "ADD",
            Operator::Sub => "SUB",
            Operator::Mul => "MUL",
            _ => "DIV",
        };
        for (left, right, result) in [
            (left, right, result),
            (long(left), long(right), long(result)),
        ] {
            let vars = format!("a : {left}; b : {right};");
            let expr = format!("{function}(a, b)");
            let found = codes(&diagnostics(&program(&vars, result, &expr), &edition_3()));
            assert!(found.is_empty(), "{expr} on {left}, {right}: {found:?}");
        }
    }
}

/// REQ-AO-analyzer-035: a call to a typed name is checked against its own
/// signature.
#[spec_test(REQ_AO_analyzer_035)]
#[test]
fn analyzer_spec_req_ao_035_typed_name_is_checked_against_its_signature() {
    let options = CompilerOptions::default();
    let clean = codes(&diagnostics(
        &program("t1 : TIME; t2 : TIME;", "TIME", "ADD_TIME(t1, t2)"),
        &options,
    ));
    assert!(clean.is_empty(), "{clean:?}");
    let mismatch = codes(&diagnostics(
        &program("t1 : TIME; r : REAL;", "TIME", "ADD_TIME(t1, r)"),
        &options,
    ));
    assert_eq!(
        mismatch,
        vec![Problem::FunctionCallArgTypeMismatch.code().to_string()]
    );
}

/// REQ-AO-analyzer-036: an arithmetic expression is reported once, while
/// the bit-string operators are still reported per operand with their
/// category.
#[spec_test(REQ_AO_analyzer_036)]
#[test]
fn analyzer_spec_req_ao_036_arithmetic_once_bit_string_family_per_operand() {
    let options = CompilerOptions::default();
    let arithmetic = diagnostics(
        &program("r1 : REAL; r2 : REAL;", "REAL", "r1 MOD r2"),
        &options,
    );
    assert_eq!(codes(&arithmetic), vec![p4049()]);

    let bitwise = diagnostics(
        &program("d1 : DINT; d2 : DINT;", "DINT", "d1 AND d2"),
        &options,
    );
    assert_eq!(codes(&bitwise), vec![p4049(), p4049()]);
    assert!(bitwise
        .iter()
        .all(|d| d.described.contains(&"expected=ANY_BIT".to_owned())));
}
