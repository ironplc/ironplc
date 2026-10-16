//! Tests of arithmetic on times and dates compiled from the lowered program,
//! with every statement required to lower.

use rstest::rstest;

use crate::common::{date, datetime, time, Duration};
use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// The typed time and date functions, and the operators that resolve to
/// them, give the values the AST route gives. A negative duration divides
/// toward zero when it is converted to seconds, a difference of dates and
/// times is in milliseconds, a `TIME` added to an `LTIME` is widened, and
/// `DT_TO_DATE` and `DT_TO_TOD` split a date and time at midnight.
#[test]
fn run_when_time_functions_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR t1 : TIME := T#1500ms; t2 : TIME := T#4s; lt : LTIME;
               s : DT := DT#1970-01-02-00:00:10; s0 : DT := DT#1970-01-01-00:00:00;
               d : DATE := D#1970-01-02; c : TOD := TOD#00:00:10.5; c2 : TOD := TOD#00:00:12;
               r1 : TIME; r2 : DT; r3 : TIME; r4 : DATE; r5 : TOD; r6 : DT; r7 : TIME;
               r8 : TIME; r9 : LTIME; r10 : TIME; r11 : DT; r12 : TIME; r13 : TIME;
               r14 : LTIME; END_VAR
           r1 := SUB_TIME(t1, t2);
           r2 := ADD_DT_TIME(s, r1);
           r3 := SUB_DT_DT(s, s0);
           r4 := DT_TO_DATE(s);
           r5 := DT_TO_TOD(s);
           r6 := CONCAT_DATE_TOD(d, c);
           r7 := MUL_TIME(t1, 3);
           r8 := DIV_TIME(t2, 2.5);
           r9 := MUL_LTIME(ADD_LTIME(lt, t1), 2);
           r10 := SUB_TOD_TOD(c, c2);
           r11 := s - t2;
           r12 := d - DT_TO_DATE(s0);
           r13 := ADD(t1, t2, t1);
           r14 := lt + t1 * 2;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("r1"), Duration::milliseconds(-2500));
    assert_eq!(snapshot.read("r2"), datetime!(1970-01-02 00:00:08));
    assert_eq!(snapshot.read("r3"), Duration::milliseconds(86_410_000));
    assert_eq!(snapshot.read("r4"), date!(1970 - 01 - 02));
    assert_eq!(snapshot.read("r5"), time!(00:00:10));
    assert_eq!(snapshot.read("r6"), datetime!(1970-01-02 00:00:10));
    assert_eq!(snapshot.read("r7"), Duration::milliseconds(4500));
    assert_eq!(snapshot.read("r8"), Duration::milliseconds(1600));
    assert_eq!(snapshot.read("r9"), Duration::milliseconds(3000));
    assert_eq!(snapshot.read("r10"), Duration::milliseconds(-1500));
    assert_eq!(snapshot.read("r11"), datetime!(1970-01-02 00:00:06));
    assert_eq!(snapshot.read("r12"), Duration::milliseconds(86_400_000));
    assert_eq!(snapshot.read("r13"), Duration::milliseconds(7000));
    assert_eq!(snapshot.read("r14"), Duration::milliseconds(3000));
}

/// A duration negates as the integer it is stored as: a positive `TIME`
/// becomes negative, negating it twice gives it back, and an `LTIME` past
/// the range of 32 bits negates at 64.
#[test]
fn run_when_duration_negated_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR t : TIME := T#1500ms; lt : LTIME;
               r1 : TIME; r2 : TIME; r3 : LTIME; END_VAR
           r1 := -t;
           r2 := -(-t);
           lt := t;
           lt := lt * 2000000;
           r3 := -lt;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("r1"), Duration::milliseconds(-1500));
    assert_eq!(snapshot.read("r2"), Duration::milliseconds(1500));
    assert_eq!(snapshot.read("r3"), Duration::milliseconds(-3_000_000_000));
}

/// A typed time function, an operator or function form that resolves to
/// one, `DT_TO_TOD`, a time or date literal and a negated duration compile
/// to the same instructions and constants by both routes. `DT_TO_DATE` is
/// left out: the AST route reads its operand twice, and the lowered route
/// once (REQ-LOW-lowering-165).
#[rstest]
#[case::add_time("t := ADD_TIME(t, t2);")]
#[case::add_tod_time("c := ADD_TOD_TIME(c, t);")]
#[case::sub_tod_tod("t := SUB_TOD_TOD(c, c);")]
#[case::add_dt_time("s := ADD_DT_TIME(s, t);")]
#[case::sub_dt_dt("t := SUB_DT_DT(s, s);")]
#[case::concat_date_tod("s := CONCAT_DATE_TOD(d, c);")]
#[case::mul_time_dint("t := MUL_TIME(t, n);")]
#[case::div_time_real("t := DIV_TIME(t, r);")]
#[case::add_ltime("lt := ADD_LTIME(lt, lt);")]
#[case::mul_ltime_lreal("lt := MUL_LTIME(lt, lr);")]
#[case::dt_to_tod("c := DT_TO_TOD(s);")]
#[case::add_time_operator("t := t + t2;")]
#[case::sub_tod_time_operator("c := c - t;")]
#[case::sub_dt_time_operator("s := s - t;")]
#[case::sub_date_date_operator("t := d - d;")]
#[case::mul_time_real_operator("t := t * r;")]
#[case::add_ltime_widened_operator("lt := lt + t;")]
#[case::add_time_fold("t := ADD(t, t2, t);")]
#[case::div_time_form("t := DIV(t, n);")]
#[case::duration_literal("t := T#1S500MS;")]
#[case::time_of_day_literal("c := TOD#10:00:00;")]
#[case::date_literal("d := D#1970-01-02;")]
#[case::date_and_time_literal("s := DT#1970-01-02-00:00:10;")]
#[case::literal_operand("s := s + T#4S;")]
#[case::negated_time("t := -t;")]
#[case::negated_ltime("lt := -lt;")]
fn compile_when_time_function_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR t : TIME; t2 : TIME; c : TOD; s : DT; d : DATE; lt : LTIME; n : DINT;
               r : REAL; lr : LREAL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}
