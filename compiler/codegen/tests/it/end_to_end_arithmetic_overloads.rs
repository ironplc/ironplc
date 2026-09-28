//! End-to-end tests for arithmetic operators on the time and date types.
//!
//! An operator on a Table 30 pair compiles through the typed routine its
//! typed call compiles through, so it computes in the units each type is
//! stored in: milliseconds for `TIME` and `TIME_OF_DAY`, seconds for `DATE`
//! and `DATE_AND_TIME` (ADR-0025). Each test is named for the requirement of
//! `specs/design/arithmetic-operator-overloads.md` it covers.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::{
    assert_run_f32, assert_run_f64, assert_run_i32, assert_run_i32_with, assert_run_i64,
    assert_run_i64_with,
};

/// REQ-AO-codegen-002: `dt + t` (here `stamp + t`) converts the duration from milliseconds to
/// seconds, as `ADD_DT_TIME` does. 2000-01-01-00:00:00 is 946684800 seconds
/// since the epoch.
#[spec_test(REQ_AO_codegen_002)]
#[rstest]
#[case::operator("stamp + t")]
#[case::typed_call("ADD_DT_TIME(stamp, t)")]
fn end_to_end_req_ao_002_when_dt_plus_time_then_adds_seconds(#[case] expr: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    result : DATE_AND_TIME;
    stamp : DATE_AND_TIME := DT#2000-01-01-00:00:00;
    t : TIME := T#1h;
  END_VAR
  result := {expr};
END_PROGRAM"
    );
    assert_run_i32(&source, &[(0, 946_688_400)]);
}

/// REQ-AO-codegen-003: `t * r` with a `REAL` factor promotes to floating
/// point, as `MUL_TIME` does.
#[spec_test(REQ_AO_codegen_003)]
#[rstest]
#[case::operator("t * r")]
#[case::typed_call("MUL_TIME(t, r)")]
fn end_to_end_req_ao_003_when_time_times_real_then_scales(#[case] expr: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    result : TIME;
    t : TIME := T#1s;
    r : REAL := 1.5;
  END_VAR
  result := {expr};
END_PROGRAM"
    );
    assert_run_i32(&source, &[(0, 1500)]);
}

/// REQ-AO-codegen-004: `d1 - d2` on `DATE` is a `TIME` in milliseconds.
#[spec_test(REQ_AO_codegen_004)]
#[rstest]
#[case::operator("d1 - d2")]
#[case::typed_call("SUB_DATE_DATE(d1, d2)")]
fn end_to_end_req_ao_004_when_date_minus_date_then_time_in_ms(#[case] expr: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    result : TIME;
    d1 : DATE := D#2000-01-02;
    d2 : DATE := D#2000-01-01;
  END_VAR
  result := {expr};
END_PROGRAM"
    );
    assert_run_i32(&source, &[(0, 86_400_000)]);
}

/// REQ-AO-codegen-005: the long forms compute at 64 bits, and a short
/// operand is widened by its signedness.
#[spec_test(REQ_AO_codegen_005)]
#[rstest]
// 60 days in milliseconds does not fit in 32 bits.
#[case::beyond_32_bits("LTIME", "lt30d + lt30d", 5_184_000_000)]
// A negative short TIME is sign-extended: 2h + (-5s).
#[case::short_time_sign_extended("LTIME", "lt2h + t", 7_195_000)]
#[case::short_time_on_left("LTIME", "t + lt2h", 7_195_000)]
// A DATE_AND_TIME past 2038 does not fit in an i32; it is zero-extended.
#[case::short_date_zero_extended("LTIME", "ldt_late - dt_late", 3_600_000)]
#[case::long_scaled_by_real("LTIME", "lt2h * r", 10_800_000)]
fn end_to_end_req_ao_005_when_long_form_then_computes_at_64_bits(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: i64,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    lt30d : LTIME := LTIME#30d;
    lt2h : LTIME := LTIME#2h;
    t : TIME := T#-5s;
    ldt_late : LDATE_AND_TIME := LDT#2100-01-01-01:00:00;
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
    r : REAL := 1.5;
  END_VAR
  result := {expr};
END_PROGRAM"
    );
    assert_run_i64_with(
        &source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(0, expected)],
    );
}

/// REQ-AO-codegen-009: an extensible call folds through the typed routine,
/// computing what the operator expression folded from the left computes.
#[spec_test(REQ_AO_codegen_009)]
#[rstest]
#[case::add_times("TIME", "ADD(t1, t2, t3)", 6000)]
#[case::add_times_operator("TIME", "t1 + t2 + t3", 6000)]
// TIME_OF_DAY + TIME + TIME: 10:00:00 + 1s + 2s.
#[case::add_tod("TIME_OF_DAY", "ADD(clock, t1, t2)", 36_003_000)]
#[case::mul_time("TIME", "MUL(t1, d, d)", 9000)]
fn end_to_end_req_ao_009_when_extensible_call_on_times_then_folds(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    t1 : TIME := T#1s;
    t2 : TIME := T#2s;
    t3 : TIME := T#3s;
    clock : TIME_OF_DAY := TOD#10:00:00;
    d : DINT := 3;
  END_VAR
  result := {expr};
END_PROGRAM"
    );
    assert_run_i32(&source, &[(0, expected)]);
}

/// REQ-AO-codegen-007: operands of different widths compute at the result
/// type, the narrower one converted first.
#[spec_test(REQ_AO_codegen_007)]
fn end_to_end_req_ao_007_when_int_plus_real_then_adds_as_real() {
    assert_run_f32(
        "
PROGRAM main
  VAR
    x : REAL;
    i : INT := 3;
    r : REAL := 1.5;
  END_VAR
  x := i + r;
END_PROGRAM",
        &[(0, 4.5)],
    );
}

/// REQ-AO-codegen-007: a `UDINT` operand of a `LINT` operation is
/// zero-extended before the 64-bit add.
#[spec_test(REQ_AO_codegen_007)]
fn end_to_end_req_ao_007_when_udint_plus_lint_then_adds_at_64_bits() {
    assert_run_i64(
        "
PROGRAM main
  VAR
    x : LINT;
    u : UDINT := 4000000000;
    l : LINT := 1;
  END_VAR
  x := u + l;
END_PROGRAM",
        &[(0, 4_000_000_001)],
    );
}

/// REQ-AO-codegen-008: a `DINT * DINT` product assigned to a `LINT` is
/// computed at 32 bits, as its operands' type, and widened: 100000 squared
/// wraps to 1410065408.
#[spec_test(REQ_AO_codegen_008)]
fn end_to_end_req_ao_008_when_dint_product_assigned_to_lint_then_wraps_at_32_bits() {
    assert_run_i64(
        "
PROGRAM main
  VAR
    l : LINT;
    d1 : DINT := 100000;
    d2 : DINT := 100000;
  END_VAR
  l := d1 * d2;
END_PROGRAM",
        &[(0, 1_410_065_408)],
    );
}

/// REQ-AO-codegen-010: `UDINT / UDINT` divides unsigned whatever the
/// target's signedness.
#[spec_test(REQ_AO_codegen_010)]
fn end_to_end_req_ao_010_when_udint_quotient_assigned_to_dint_then_divides_unsigned() {
    assert_run_i32(
        "
PROGRAM main
  VAR
    d : DINT;
    u1 : UDINT := 4000000000;
    u2 : UDINT := 2;
  END_VAR
  d := u1 / u2;
END_PROGRAM",
        &[(0, 2_000_000_000)],
    );
}

/// REQ-AO-codegen-011: the function form computes each fold step as the
/// operator expression does.
#[spec_test(REQ_AO_codegen_011)]
#[rstest]
#[case::operator("i + r")]
#[case::function_form("ADD(i, r)")]
fn end_to_end_req_ao_011_when_add_call_on_int_and_real_then_adds_as_real(#[case] expr: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : REAL;
    i : INT := 3;
    r : REAL := 1.5;
  END_VAR
  x := {expr};
END_PROGRAM"
    );
    assert_run_f32(&source, &[(0, 4.5)]);
}

/// REQ-AO-codegen-011: an extensible call widens step by step: INT + REAL is
/// REAL, and REAL + LREAL is LREAL.
#[spec_test(REQ_AO_codegen_011)]
fn end_to_end_req_ao_011_when_add_call_widens_per_step_then_computes_at_widest() {
    assert_run_f64(
        "
PROGRAM main
  VAR
    x : LREAL;
    i : INT := 3;
    r : REAL := 1.5;
    lr : LREAL := 2.25;
  END_VAR
  x := ADD(i, r, lr);
END_PROGRAM",
        &[(0, 6.75)],
    );
}

/// Bit-string arithmetic under a dialect that allows it computes at the bit
/// string's width: `BYTE#255 + 1` wraps to 0.
#[test]
fn end_to_end_when_byte_plus_one_under_codesys_then_wraps() {
    assert_run_i32_with(
        "
PROGRAM main
  VAR
    b : BYTE := 255;
  END_VAR
  b := b + 1;
END_PROGRAM",
        &CompilerOptions::from_dialect(Dialect::Codesys),
        &[(0, 0)],
    );
}
