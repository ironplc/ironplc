//! Spec conformance tests for the arithmetic operator overloads
//! (analyzer-owned requirements).
//!
//! These cover the resolver, `REQ-AO-analyzer-001` to `-015`. They are
//! plain tests named for their requirement until the design is listed in
//! this crate's `build.rs`, which happens once every analyzer-owned
//! requirement has a test; the attribute then becomes
//! `#[spec_test(REQ_AO_analyzer_NNN)]`.
//!
//! See `specs/design/arithmetic-operator-overloads.md`.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::Operator;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, resolve_with, FoldFailure, Overload,
};
use crate::intermediates::operator_function_form::operator_function_form;
use crate::intermediates::stdlib_function::get_all_stdlib_functions;
use crate::intermediates::stdlib_time_function::long_form;

const ANY_NUM_TYPES: [&str; 10] = [
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL",
];

fn ty(name: &str) -> TypeName {
    TypeName::from(name)
}

fn numeric(result: &str) -> Option<Overload> {
    Some(Overload::Numeric { result: ty(result) })
}

fn typed(name: &'static str, result: &str) -> Option<Overload> {
    Some(Overload::Typed {
        name,
        result: ty(result),
    })
}

/// Resolves `left OP right` under default options.
fn resolve(op: &Operator, left: &str, right: &str) -> Option<Overload> {
    resolve_arithmetic_overload(
        op,
        Some(&ty(left)),
        Some(&ty(right)),
        &CompilerOptions::default(),
    )
}

/// Resolves `left OP right` with the bit-string arithmetic rule on or off.
fn resolve_bit_strings(op: &Operator, left: &str, right: &str, allowed: bool) -> Option<Overload> {
    resolve_with(
        op,
        Some(&ty(left)),
        Some(&ty(right)),
        &CompilerOptions::default(),
        allowed,
    )
}

/// The four overloaded operators.
fn overloaded_operators() -> [Operator; 4] {
    [Operator::Add, Operator::Sub, Operator::Mul, Operator::Div]
}

/// REQ-AO-analyzer-001: two operands of one numeric type resolve to the
/// numeric overload with that type.
#[rstest]
fn analyzer_spec_req_ao_001_same_numeric_type_resolves_to_it(
    #[values(Operator::Add, Operator::Sub, Operator::Mul, Operator::Div)] op: Operator,
    #[values(
        "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL"
    )]
    operand: &str,
) {
    assert_eq!(resolve(&op, operand, operand), numeric(operand));
}

/// REQ-AO-analyzer-002: one operand widens to the other, and the wider
/// type is the result.
#[rstest]
#[case::int_dint("INT", "DINT", "DINT")]
#[case::dint_int("DINT", "INT", "DINT")]
#[case::int_real("INT", "REAL", "REAL")]
#[case::udint_lint("UDINT", "LINT", "LINT")]
#[case::real_lreal("REAL", "LREAL", "LREAL")]
#[case::usint_int("USINT", "INT", "INT")]
fn analyzer_spec_req_ao_002_widening_pair_resolves_to_wider_type(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result: &str,
) {
    for op in overloaded_operators() {
        assert_eq!(resolve(&op, left, right), numeric(result), "{op}");
    }
}

/// REQ-AO-analyzer-003: a bare integer literal takes the other operand's
/// numeric type, including a real one.
#[rstest]
#[case::dint_literal("DINT", "ANY_INT", "DINT")]
#[case::literal_real("ANY_INT", "REAL", "REAL")]
#[case::lreal_literal("LREAL", "ANY_INT", "LREAL")]
fn analyzer_spec_req_ao_003_integer_literal_takes_other_operand_type(
    #[case] left: &str,
    #[case] right: &str,
    #[case] result: &str,
) {
    assert_eq!(resolve(&Operator::Add, left, right), numeric(result));
}

/// REQ-AO-analyzer-004: two numeric types where neither widens to the
/// other do not resolve.
#[rstest]
#[case::dint_real("DINT", "REAL")]
#[case::dint_udint("DINT", "UDINT")]
#[case::dint_real_literal("DINT", "ANY_REAL")]
fn analyzer_spec_req_ao_004_non_widening_pair_does_not_resolve(
    #[case] left: &str,
    #[case] right: &str,
) {
    for op in overloaded_operators() {
        assert_eq!(resolve(&op, left, right), None, "{op}");
    }
}

/// REQ-AO-analyzer-005: each Table 30 pair resolves to its typed name with
/// the typed function's return type.
#[rstest]
#[case::add_time(Operator::Add, "TIME", "TIME", "ADD_TIME", "TIME")]
#[case::add_tod_time(Operator::Add, "TIME_OF_DAY", "TIME", "ADD_TOD_TIME", "TIME_OF_DAY")]
#[case::add_dt_time(Operator::Add, "DATE_AND_TIME", "TIME", "ADD_DT_TIME", "DATE_AND_TIME")]
#[case::sub_time(Operator::Sub, "TIME", "TIME", "SUB_TIME", "TIME")]
#[case::sub_date_date(Operator::Sub, "DATE", "DATE", "SUB_DATE_DATE", "TIME")]
#[case::sub_tod_time(Operator::Sub, "TIME_OF_DAY", "TIME", "SUB_TOD_TIME", "TIME_OF_DAY")]
#[case::sub_tod_tod(Operator::Sub, "TIME_OF_DAY", "TIME_OF_DAY", "SUB_TOD_TOD", "TIME")]
#[case::sub_dt_time(Operator::Sub, "DATE_AND_TIME", "TIME", "SUB_DT_TIME", "DATE_AND_TIME")]
#[case::sub_dt_dt(Operator::Sub, "DATE_AND_TIME", "DATE_AND_TIME", "SUB_DT_DT", "TIME")]
#[case::mul_time(Operator::Mul, "TIME", "DINT", "MUL_TIME", "TIME")]
#[case::div_time(Operator::Div, "TIME", "DINT", "DIV_TIME", "TIME")]
fn analyzer_spec_req_ao_005_table_30_pair_resolves_to_typed_name(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
    #[case] name: &'static str,
    #[case] result: &str,
) {
    assert_eq!(resolve(&op, left, right), typed(name, result));
    let registered = get_all_stdlib_functions()
        .into_iter()
        .find(|sig| sig.name == Id::from(name))
        .unwrap();
    assert_eq!(
        registered.return_type.unwrap().to_type_name(),
        ty(result),
        "the resolver's result type is the registered return type"
    );
}

/// REQ-AO-analyzer-006: each long-width pair resolves to the long form,
/// not the short one.
#[rstest]
#[case::add_ltime(Operator::Add, "LTIME", "LTIME", "ADD_LTIME", "LTIME")]
#[case::add_ltod_ltime(
    Operator::Add,
    "LTIME_OF_DAY",
    "LTIME",
    "ADD_LTOD_LTIME",
    "LTIME_OF_DAY"
)]
#[case::add_ldt_ltime(
    Operator::Add,
    "LDATE_AND_TIME",
    "LTIME",
    "ADD_LDT_LTIME",
    "LDATE_AND_TIME"
)]
#[case::sub_ltime(Operator::Sub, "LTIME", "LTIME", "SUB_LTIME", "LTIME")]
#[case::sub_ldate_ldate(Operator::Sub, "LDATE", "LDATE", "SUB_LDATE_LDATE", "LTIME")]
#[case::sub_ltod_ltime(
    Operator::Sub,
    "LTIME_OF_DAY",
    "LTIME",
    "SUB_LTOD_LTIME",
    "LTIME_OF_DAY"
)]
#[case::sub_ltod_ltod(
    Operator::Sub,
    "LTIME_OF_DAY",
    "LTIME_OF_DAY",
    "SUB_LTOD_LTOD",
    "LTIME"
)]
#[case::sub_ldt_ltime(
    Operator::Sub,
    "LDATE_AND_TIME",
    "LTIME",
    "SUB_LDT_LTIME",
    "LDATE_AND_TIME"
)]
#[case::sub_ldt_ldt(
    Operator::Sub,
    "LDATE_AND_TIME",
    "LDATE_AND_TIME",
    "SUB_LDT_LDT",
    "LTIME"
)]
#[case::mul_ltime(Operator::Mul, "LTIME", "DINT", "MUL_LTIME", "LTIME")]
#[case::div_ltime(Operator::Div, "LTIME", "DINT", "DIV_LTIME", "LTIME")]
fn analyzer_spec_req_ao_006_long_pair_resolves_to_long_form(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
    #[case] name: &'static str,
    #[case] result: &str,
) {
    assert_eq!(resolve(&op, left, right), typed(name, result));
}

/// REQ-AO-analyzer-007: a pair mixing the widths of one family resolves to
/// the long form with the long result.
#[rstest]
#[case::time_ltime(Operator::Add, "TIME", "LTIME", "ADD_LTIME", "LTIME")]
#[case::ltime_time(Operator::Add, "LTIME", "TIME", "ADD_LTIME", "LTIME")]
#[case::dt_ltime(
    Operator::Add,
    "DATE_AND_TIME",
    "LTIME",
    "ADD_LDT_LTIME",
    "LDATE_AND_TIME"
)]
#[case::ldt_dt(
    Operator::Sub,
    "LDATE_AND_TIME",
    "DATE_AND_TIME",
    "SUB_LDT_LDT",
    "LTIME"
)]
fn analyzer_spec_req_ao_007_mixed_width_pair_resolves_to_long_form(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
    #[case] name: &'static str,
    #[case] result: &str,
) {
    assert_eq!(resolve(&op, left, right), typed(name, result));
}

/// REQ-AO-analyzer-008: TIME scaled by any ANY_NUM operand resolves, and a
/// number scaled by TIME does not.
#[rstest]
fn analyzer_spec_req_ao_008_time_scaled_by_number_resolves(
    #[values(Operator::Mul, Operator::Div)] op: Operator,
    #[values(
        "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL",
        "ANY_INT", "ANY_REAL"
    )]
    scale: &str,
) {
    let name = if op == Operator::Mul {
        "MUL_TIME"
    } else {
        "DIV_TIME"
    };
    assert_eq!(resolve(&op, "TIME", scale), typed(name, "TIME"));
    assert_eq!(resolve(&op, scale, "TIME"), None, "{scale} {op} TIME");
}

/// REQ-AO-analyzer-009: pairs no row covers do not resolve.
#[rstest]
#[case::string(Operator::Add, "STRING", "STRING")]
#[case::bool(Operator::Add, "BOOL", "BOOL")]
#[case::time_date(Operator::Add, "TIME", "DATE")]
#[case::date_date(Operator::Add, "DATE", "DATE")]
#[case::time_mul_time(Operator::Mul, "TIME", "TIME")]
#[case::time_div_time(Operator::Div, "TIME", "TIME")]
fn analyzer_spec_req_ao_009_uncovered_pair_does_not_resolve(
    #[case] op: Operator,
    #[case] left: &str,
    #[case] right: &str,
) {
    assert_eq!(resolve(&op, left, right), None);
}

/// REQ-AO-analyzer-010: with bit-string arithmetic, a bit string is judged
/// as its unsigned integer and the result follows the design's rule.
#[rstest]
#[case::byte_byte("BYTE", "BYTE", numeric("BYTE"))]
#[case::byte_word("BYTE", "WORD", numeric("WORD"))]
#[case::byte_literal("BYTE", "ANY_INT", numeric("BYTE"))]
#[case::byte_int("BYTE", "INT", numeric("INT"))]
#[case::byte_real("BYTE", "REAL", numeric("REAL"))]
#[case::word_int("WORD", "INT", None)]
#[case::bool_bool("BOOL", "BOOL", None)]
#[case::bool_int("BOOL", "INT", None)]
fn analyzer_spec_req_ao_010_bit_string_judged_as_unsigned_integer_with_rule(
    #[case] left: &str,
    #[case] right: &str,
    #[case] expected: Option<Overload>,
) {
    for op in overloaded_operators() {
        assert_eq!(
            resolve_bit_strings(&op, left, right, true),
            expected,
            "{op}"
        );
    }
}

/// REQ-AO-analyzer-011: without the rule, a bit-string operand does not
/// resolve.
#[rstest]
#[case::byte_byte("BYTE", "BYTE")]
#[case::byte_word("BYTE", "WORD")]
#[case::byte_literal("BYTE", "ANY_INT")]
#[case::byte_int("BYTE", "INT")]
#[case::byte_real("BYTE", "REAL")]
#[case::lword_lint("LWORD", "LINT")]
fn analyzer_spec_req_ao_011_bit_string_does_not_resolve_without_rule(
    #[case] left: &str,
    #[case] right: &str,
) {
    for op in overloaded_operators() {
        assert_eq!(resolve_bit_strings(&op, left, right, false), None, "{op}");
        assert_eq!(
            resolve(&op, left, right),
            None,
            "{op} under default options"
        );
    }
}

/// REQ-AO-analyzer-012: an operand the predicate cannot judge resolves as
/// unchecked, which is not the numeric overload.
#[rstest]
#[case::subrange(Some("Pct"), Some("INT"))]
#[case::enumeration(Some("INT"), Some("Color"))]
#[case::missing_left(None, Some("INT"))]
#[case::missing_right(Some("TIME"), None)]
fn analyzer_spec_req_ao_012_unjudgeable_operand_resolves_as_unchecked(
    #[case] left: Option<&str>,
    #[case] right: Option<&str>,
) {
    let left = left.map(ty);
    let right = right.map(ty);
    let overload = resolve_arithmetic_overload(
        &Operator::Add,
        left.as_ref(),
        right.as_ref(),
        &CompilerOptions::default(),
    );
    assert_eq!(overload, Some(Overload::Unchecked));
    assert!(!matches!(overload, Some(Overload::Numeric { .. })));
}

/// REQ-AO-analyzer-013: an extensible call folds from the left, and a
/// failing step names its operand types.
#[test]
fn analyzer_spec_req_ao_013_extensible_call_folds_from_the_left() {
    let options = CompilerOptions::default();
    let time = ty("TIME");
    let real = ty("REAL");
    let inputs = [Some(&time), Some(&time), Some(&time)];
    assert_eq!(
        resolve_arithmetic_fold(&Operator::Add, &inputs, &options),
        Ok(Overload::Typed {
            name: "ADD_TIME",
            result: ty("TIME")
        })
    );
    let inputs = [Some(&time), Some(&time), Some(&real)];
    assert_eq!(
        resolve_arithmetic_fold(&Operator::Add, &inputs, &options),
        Err(FoldFailure {
            step: 2,
            left: ty("TIME"),
            right: ty("REAL"),
        })
    );
}

/// REQ-AO-analyzer-014: every typed name in the table, in both widths, is
/// a registered two-input signature.
#[rstest]
fn analyzer_spec_req_ao_014_typed_names_are_registered_two_input_functions(
    #[values("ADD", "SUB", "MUL", "DIV")] function: &str,
) {
    let registered = get_all_stdlib_functions();
    let form = operator_function_form(function).unwrap();
    assert!(
        !form.typed_overloads().is_empty(),
        "{function} has overloads"
    );
    for short in form.typed_overloads() {
        let long = long_form(short).unwrap_or_else(|| panic!("{short} has a long form"));
        for name in [*short, long] {
            let sig = registered
                .iter()
                .find(|sig| sig.name == Id::from(name))
                .unwrap_or_else(|| panic!("{name} is registered"));
            assert_eq!(sig.parameters.len(), 2, "{name} takes two inputs");
            assert!(!sig.is_extensible, "{name} is not extensible");
        }
    }
}

/// REQ-AO-analyzer-015: the bit-string rule does not apply to MOD, so
/// `b MOD 2` on a BYTE stays unresolved with the rule on.
#[test]
fn analyzer_spec_req_ao_015_bit_string_rule_does_not_apply_to_mod() {
    assert_eq!(
        resolve_bit_strings(&Operator::Mod, "BYTE", "ANY_INT", true),
        None
    );
    assert_eq!(
        resolve_bit_strings(&Operator::Mod, "BYTE", "BYTE", true),
        None
    );
    assert_eq!(
        resolve_bit_strings(&Operator::Mod, "DINT", "ANY_INT", true),
        numeric("DINT")
    );
}

/// The numeric overload's category is the row's: MOD admits integers only.
#[test]
fn resolve_arithmetic_overload_when_mod_of_reals_then_none() {
    assert_eq!(resolve(&Operator::Mod, "REAL", "REAL"), None);
    for operand in ANY_NUM_TYPES.iter().filter(|t| !t.ends_with("REAL")) {
        assert_eq!(resolve(&Operator::Mod, operand, operand), numeric(operand));
    }
}
