//! End-to-end tests for the type a comparison compares its operands at
//! (#1920): the type one operand widens to, whichever side it is on, with
//! the narrower operand widened by its own signedness.
//!
//! Each pair of operands differs only above the narrower type's width, so a
//! comparison computed at the narrower type gives the wrong answer. See
//! `specs/design/comparison-operand-type.md`.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::assert_run_with;

/// A program assigning `expr` to `result`.
///
/// Each wide operand, truncated to 32 bits, is no greater than the narrow
/// operand it is compared with: `l` is 2^32 + 1, `lt_wide` 2^32 + 1 ms, and
/// `ld_2107` and `ldt_2107` are past 2106, the last year a 32-bit count of
/// seconds reaches.
fn program(expr: &str) -> String {
    format!(
        "
FUNCTION late_udint : UDINT
  late_udint := 4000000000;
END_FUNCTION
PROGRAM main
  VAR
    result : BOOL;
    i : DINT := 1;
    l : LINT := 4294967297;
    l2 : LINT := 8589934593;
    ud : UDINT := 1;
    ul : ULINT := 4294967297;
    ul_late : ULINT := 4000000000;
    dw : DWORD := 1;
    lw : LWORD := 16#1_0000_0001;
    lw_ones : LWORD := 16#FFFF_FFFF;
    dw_ones : DWORD := 16#FFFF_FFFF;
    n : INT := 1;
    r : REAL := 1.0;
    r_half : REAL := 1.5;
    lr : LREAL := 1.00000001;
    t_1ms : TIME := T#1ms;
    t_neg : TIME := T#-5s;
    lt_wide : LTIME := LTIME#49d17h2m47s297ms;
    lt_zero : LTIME := LTIME#0s;
    d_2000 : DATE := D#2000-01-01;
    ld_2107 : LDATE := LDATE#2107-01-01;
    dt_2000 : DATE_AND_TIME := DT#2000-01-01-00:00:00;
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
    ldt_2107 : LDATE_AND_TIME := LDT#2107-01-01-00:00:00;
    ldt_early : LDATE_AND_TIME := LDT#1970-01-01-00:00:01;
    s_abc : STRING := 'abc';
    s_abd : STRING := 'abd';
  END_VAR
  result := {expr};
END_PROGRAM
"
    )
}

fn assert_result(expr: &str, expected: i64) {
    assert_run_with::<i64>(
        &program(expr),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[("result", expected)],
    );
}

#[spec_test(REQ_CMP_codegen_001)]
#[rstest]
#[case::lt_narrow_left("i < l", 1)]
#[case::gt_narrow_right("l > i", 1)]
#[case::le_narrow_left("i <= l", 1)]
#[case::ge_narrow_left("i >= l", 0)]
#[case::eq_narrow_left("i = l", 0)]
#[case::ne_narrow_left("i <> l", 1)]
#[case::gt_narrow_left("i > l", 0)]
#[case::unsigned_narrow_left("ud < ul", 1)]
#[case::unsigned_into_signed("ud < l", 1)]
fn end_to_end_when_integers_compared_then_computed_at_wider_type(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(expr, expected);
}

#[spec_test(REQ_CMP_codegen_002)]
#[rstest]
#[case::udint_function_result("ul_late = late_udint()", 1)]
#[case::udint_function_result_left("late_udint() = ul_late", 1)]
#[case::dword_narrow_left("dw < lw", 1)]
#[case::dword_not_equal("dw = lw", 0)]
#[case::dword_all_ones("dw_ones = lw_ones", 1)]
#[case::dword_expression("lw_ones = (dw_ones AND dw_ones)", 1)]
fn end_to_end_when_unsigned_narrow_operand_then_zero_extended(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(expr, expected);
}

#[spec_test(REQ_CMP_codegen_003)]
#[rstest]
#[case::real_narrow_left("r < lr", 1)]
#[case::real_equal("r = lr", 0)]
#[case::real_narrow_right("lr > r", 1)]
#[case::int_below_real("n < r_half", 1)]
fn end_to_end_when_real_compared_with_wider_then_converted(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(expr, expected);
}

#[spec_test(REQ_CMP_codegen_004)]
#[rstest]
#[case::time_narrow_left("t_1ms < lt_wide", 1)]
#[case::time_equal("t_1ms = lt_wide", 0)]
#[case::time_negative_sign_extended("lt_zero > t_neg", 1)]
#[case::time_negative_left("t_neg < lt_zero", 1)]
#[case::date_narrow_left("d_2000 < ld_2107", 1)]
#[case::date_and_time_narrow_left("dt_2000 < ldt_2107", 1)]
#[case::date_and_time_narrow_right("ldt_2107 > dt_2000", 1)]
#[case::date_and_time_after_2038_zero_extended("ldt_early < dt_late", 1)]
fn end_to_end_when_short_temporal_compared_with_long_then_widened(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(expr, expected);
}

#[spec_test(REQ_CMP_codegen_005)]
#[rstest]
#[case::lt("LT(i, l)", 1)]
#[case::gt("GT(l, i)", 1)]
#[case::le("LE(i, l)", 1)]
#[case::ge("GE(i, l)", 0)]
#[case::eq("EQ(i, l)", 0)]
#[case::ne("NE(i, l)", 1)]
#[case::same_long_type("GT(l2, l)", 1)]
#[case::real("LT(r, lr)", 1)]
#[case::date_and_time("LT(dt_2000, ldt_2107)", 1)]
#[case::string_equal("EQ(s_abc, s_abc)", 1)]
#[case::string_not_equal("NE(s_abc, s_abc)", 0)]
#[case::string_less("LT(s_abc, s_abd)", 1)]
#[case::string_greater("GT(s_abc, s_abd)", 0)]
fn end_to_end_when_comparison_function_then_computed_at_operand_type(
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_result(expr, expected);
}

/// A condition compares through the same path as an assigned comparison.
#[test]
fn end_to_end_when_comparison_in_if_condition_then_computed_at_wider_type() {
    assert_result("FALSE;\n  IF i < l THEN result := TRUE; END_IF", 1);
}
