//! Tests of the typed time and date functions compiled from the lowered
//! program, with every statement required to lower.

use rstest::rstest;

use crate::common::{date, datetime, time, Duration};
use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// The typed time and date functions give the values the AST route gives.
/// A negative duration divides toward zero when it is converted to seconds,
/// a difference of dates and times is in milliseconds, a `TIME` added to an
/// `LTIME` is widened, and `DT_TO_DATE` and `DT_TO_TOD` split a date and time
/// at midnight.
#[test]
fn run_when_time_functions_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR t1 : TIME := T#1500ms; t2 : TIME := T#4s; lt : LTIME;
               s : DT := DT#1970-01-02-00:00:10; s0 : DT := DT#1970-01-01-00:00:00;
               d : DATE := D#1970-01-02; c : TOD := TOD#00:00:10.5; c2 : TOD := TOD#00:00:12;
               r1 : TIME; r2 : DT; r3 : TIME; r4 : DATE; r5 : TOD; r6 : DT; r7 : TIME;
               r8 : TIME; r9 : LTIME; r10 : TIME; END_VAR
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
}

/// A typed time function and `DT_TO_TOD` compile to the same instructions
/// and constants by both routes. `DT_TO_DATE` is left out: the AST route
/// reads its operand twice, and the lowered route once
/// (REQ-LOW-lowering-165).
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
