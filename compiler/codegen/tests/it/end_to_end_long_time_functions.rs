//! End-to-end tests for the long forms of the typed time and date functions
//! (`ADD_LTIME`, `SUB_LDATE_LDATE`, `MUL_LTIME`, ...).
//!
//! A long form is its short form's sequence at 64-bit width over the long
//! types, which store the same units as the short ones (ADR-0021,
//! ADR-0025): durations and times of day in milliseconds, dates in seconds.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::assert_run_i64_with;

/// A program assigning `expr` to `result` of `result_type`, the first
/// variable (index 0), with operands of every long and short temporal type.
fn program(result_type: &str, expr: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    result : {result_type};
    lt1 : LTIME := LTIME#2h;
    lt2 : LTIME := LTIME#30m;
    ltod1 : LTIME_OF_DAY := LTOD#10:00:00;
    ltod2 : LTIME_OF_DAY := LTOD#08:30:00;
    ld1 : LDATE := LDATE#2000-01-02;
    ld2 : LDATE := LDATE#2000-01-01;
    ldt1 : LDATE_AND_TIME := LDT#2000-01-01-01:00:00;
    ldt2 : LDATE_AND_TIME := LDT#2000-01-01-00:00:00;
    ldt_late : LDATE_AND_TIME := LDT#2100-01-01-01:00:00;
    dt_late : DATE_AND_TIME := DT#2100-01-01-00:00:00;
    t : TIME := T#-5s;
    d : DINT := 3;
    u : UDINT := 4;
    l : LINT := 4;
    r : REAL := 1.5;
    lr : LREAL := 2.5;
  END_VAR
  result := {expr};
END_PROGRAM
"
    )
}

#[rstest]
#[case::add_ltime("LTIME", "ADD_LTIME(lt1, lt2)", 9_000_000)]
// 60 days in milliseconds does not fit in 32 bits.
#[case::add_ltime_beyond_32_bits("LTIME", "ADD_LTIME(LTIME#30d, LTIME#30d)", 5_184_000_000)]
// A short TIME operand is sign-extended: 2h + (-5s).
#[case::add_ltime_short_negative_operand("LTIME", "ADD_LTIME(lt1, t)", 7_195_000)]
#[case::sub_ltime("LTIME", "SUB_LTIME(lt1, lt2)", 5_400_000)]
#[case::mul_ltime_by_dint("LTIME", "MUL_LTIME(lt1, d)", 21_600_000)]
#[case::mul_ltime_by_udint("LTIME", "MUL_LTIME(lt1, u)", 28_800_000)]
#[case::mul_ltime_by_lint("LTIME", "MUL_LTIME(lt1, l)", 28_800_000)]
#[case::mul_ltime_by_real("LTIME", "MUL_LTIME(lt1, r)", 10_800_000)]
#[case::mul_ltime_by_lreal("LTIME", "MUL_LTIME(lt1, lr)", 18_000_000)]
#[case::mul_ltime_beyond_32_bits("LTIME", "MUL_LTIME(LTIME#30d, d)", 7_776_000_000)]
#[case::div_ltime_by_dint("LTIME", "DIV_LTIME(lt1, d)", 2_400_000)]
#[case::div_ltime_by_real("LTIME", "DIV_LTIME(lt1, r)", 4_800_000)]
#[case::add_ltod_ltime("LTIME_OF_DAY", "ADD_LTOD_LTIME(ltod1, lt2)", 37_800_000)]
#[case::sub_ltod_ltime("LTIME_OF_DAY", "SUB_LTOD_LTIME(ltod1, lt2)", 34_200_000)]
#[case::sub_ltod_ltod("LTIME", "SUB_LTOD_LTOD(ltod1, ltod2)", 5_400_000)]
// 2000-01-01-00:00:00 is 946684800 seconds since the epoch.
#[case::add_ldt_ltime("LDATE_AND_TIME", "ADD_LDT_LTIME(ldt2, LTIME#1h)", 946_688_400)]
#[case::sub_ldt_ltime("LDATE_AND_TIME", "SUB_LDT_LTIME(ldt1, LTIME#1h)", 946_684_800)]
#[case::sub_ldt_ldt("LTIME", "SUB_LDT_LDT(ldt1, ldt2)", 3_600_000)]
#[case::sub_ldate_ldate("LTIME", "SUB_LDATE_LDATE(ld1, ld2)", 86_400_000)]
// A short DATE_AND_TIME past 2038 does not fit in an i32, so it must be
// zero-extended, not sign-extended, to meet a long operand.
#[case::sub_ldt_ldt_short_operand_after_2038("LTIME", "SUB_LDT_LDT(ldt_late, dt_late)", 3_600_000)]
fn end_to_end_when_long_typed_time_function_then_computes_at_64_bits(
    #[case] result_type: &str,
    #[case] expr: &str,
    #[case] expected: i64,
) {
    assert_run_i64_with(
        &program(result_type, expr),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(0, expected)],
    );
}
