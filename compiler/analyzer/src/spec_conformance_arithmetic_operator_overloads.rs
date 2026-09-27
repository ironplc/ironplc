//! Spec conformance tests for arithmetic operator overloads (analyzer-owned
//! requirements).
//!
//! Each test is named for the requirement it covers. The design is not yet
//! listed in `build.rs`, so the tests carry `#[test]`; they take
//! `#[spec_test(REQ_AO_analyzer_NNN)]` when the requirements are enforced.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::Operator;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, FoldFailure, Overload,
};
use crate::intermediates::operator_function_form::operator_function_form;
use crate::intermediates::stdlib_function::get_all_stdlib_functions;
use crate::intermediates::stdlib_time_function::long_form;

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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
