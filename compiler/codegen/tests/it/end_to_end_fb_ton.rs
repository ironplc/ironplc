//! End-to-end tests for function block invocation (TON on-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TON function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, parse_and_compile, FbStep, FbStep::*};

// Plain TIME variable (no timer): elapsed=var0.
const TON_TIME_ONLY: &str = "
PROGRAM main
  VAR
    elapsed : TIME;
  END_VAR
  elapsed := T#5s;
END_PROGRAM
";

// timer=var0, result=var1, with IN wired FALSE.
const TON_IN_FALSE: &str = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := FALSE, PT := T#5s, Q => result);
END_PROGRAM
";

// timer=var0, result=var1, with IN wired TRUE.
const TON_IN_TRUE: &str = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";

// timer=var0, elapsed=var1.
const TON_ET: &str = "
PROGRAM main
  VAR
    timer : TON;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s, ET => elapsed);
END_PROGRAM
";

// timer=var0, enable=var1, result=var2, elapsed=var3.
const TON_ENABLE: &str = "
PROGRAM main
  VAR
    timer : TON;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";

// timer1=var0, timer2=var1, q1=var2, q2=var3.
const TON_TWO: &str = "
PROGRAM main
  VAR
    timer1 : TON;
    timer2 : TON;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := TRUE, PT := T#3s, Q => q1);
  timer2(IN := TRUE, PT := T#7s, Q => q2);
END_PROGRAM
";

// Dot-access read of Q: Button=var0, Buzzer=var1, PulseTimer=var2.
const TON_DOT_READ_Q: &str = "
PROGRAM main
  VAR
    Button : BOOL;
    Buzzer : BOOL;
    PulseTimer : TON;
  END_VAR
  PulseTimer(IN := NOT Button, PT := T#500ms);
  Buzzer := PulseTimer.Q;
END_PROGRAM
";

// Dot-access read of ET: timer=var0, elapsed=var1.
const TON_DOT_READ_ET: &str = "
PROGRAM main
  VAR
    timer : TON;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s);
  elapsed := timer.ET;
END_PROGRAM
";

// Dot-access writes of inputs, bare call, dot-access read: timer=var0, Buzzer=var1.
const TON_DOT_WRITE_500MS: &str = "
PROGRAM main
  VAR
    timer : TON;
    Buzzer : BOOL;
  END_VAR
  timer.IN := TRUE;
  timer.PT := T#500ms;
  timer();
  Buzzer := timer.Q;
END_PROGRAM
";

// Same as above but PT set to 2s via dot-access: timer=var0, Buzzer=var1.
const TON_DOT_WRITE_2S: &str = "
PROGRAM main
  VAR
    timer : TON;
    Buzzer : BOOL;
  END_VAR
  timer.IN := TRUE;
  timer.PT := T#2s;
  timer();
  Buzzer := timer.Q;
END_PROGRAM
";

// Dot-access read of Q as an IF condition: timer=var0, done=var1.
const TON_DOT_READ_Q_IN_IF: &str = "
PROGRAM main
  VAR
    timer : TON;
    done : BOOL := FALSE;
  END_VAR
  timer(IN := TRUE, PT := T#500ms);
  IF timer.Q THEN
    done := TRUE;
  END_IF;
END_PROGRAM
";

// Dot-access read of Q inside a boolean expression: timer=var0, gate=var1,
// result=var2.
const TON_DOT_READ_Q_IN_EXPR: &str = "
PROGRAM main
  VAR
    timer : TON;
    gate : BOOL;
    result : BOOL := FALSE;
  END_VAR
  timer(IN := TRUE, PT := T#500ms);
  IF gate AND NOT timer.Q THEN
    result := TRUE;
  ELSE
    result := FALSE;
  END_IF;
END_PROGRAM
";

// Dot-access read of the TIME output ET in a comparison: timer=var0,
// past=var1.
const TON_DOT_READ_ET_IN_IF: &str = "
PROGRAM main
  VAR
    timer : TON;
    past : BOOL := FALSE;
  END_VAR
  timer(IN := TRUE, PT := T#10s);
  IF timer.ET > T#2s THEN
    past := TRUE;
  END_IF;
END_PROGRAM
";

// Dot-access read of Q as a WHILE condition guard: timer=var0, count=var1.
const TON_DOT_READ_Q_IN_WHILE: &str = "
PROGRAM main
  VAR
    timer : TON;
    count : DINT := 0;
  END_VAR
  timer(IN := TRUE, PT := T#500ms);
  WHILE NOT timer.Q AND count < 3 DO
    count := count + 1;
  END_WHILE;
END_PROGRAM
";

// Member initializer on the instance declaration sets PT, so the invocation
// need not pass it: timer=var0, result=var1. This is the shape
// docs/reference/compiler/problems/P4043.rst offers as the remedy for P4043.
const TON_MEMBER_INIT_PT: &str = "
PROGRAM main
  VAR
    timer : TON := (PT := T#5s);
    result : BOOL;
  END_VAR
  timer(IN := TRUE, Q => result);
END_PROGRAM
";

// A member initializer that an invocation later overrides: the invocation's
// own PT wins, because it is stored on every scan. timer=var0, result=var1.
const TON_MEMBER_INIT_PT_OVERRIDDEN: &str = "
PROGRAM main
  VAR
    timer : TON := (PT := T#5s);
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#1s, Q => result);
END_PROGRAM
";

#[rstest]
// Plain TIME literal assignment: T#5s stored as 5000 ms (i32).
#[case::time_value_i32_ms(TON_TIME_ONLY, &[Run(0), Expect(0, 5000)])]
// IN FALSE: Q stays FALSE.
#[case::not_triggered(TON_IN_FALSE, &[Run(0), Expect(1, 0)])]
// Before PT elapses, Q is FALSE.
#[case::triggered_before_pt(TON_IN_TRUE, &[
    Run(0), Expect(1, 0), Run(2_000_000), Expect(1, 0),
])]
// After PT elapses, Q is TRUE.
#[case::triggered_after_pt(TON_IN_TRUE, &[Run(0), Run(6_000_000), Expect(1, 1)])]
// ET reports 3s of elapsed on-delay.
#[case::reads_et(TON_ET, &[Run(0), Run(3_000_000), Expect(1, 3000)])]
// IN dropping resets the timer, which then restarts from the new rising edge.
#[case::in_reset_restarts(TON_ENABLE, &[
    Write(1, 1), Run(0), Expect(2, 0),
    Run(3_000_000), Expect(2, 0),
    Write(1, 0), Run(4_000_000), Expect(2, 0), Expect(3, 0),
    Write(1, 1), Run(6_000_000), Expect(2, 0),
    Run(10_000_000), Expect(2, 0),
    Run(12_000_000), Expect(2, 1),
])]
// ET == PT exactly: Q is TRUE.
#[case::at_exact_pt(TON_IN_TRUE, &[Run(0), Run(5_000_000), Expect(1, 1)])]
// Two TON timers with different PT run independently.
#[case::two_timers(TON_TWO, &[
    Run(0), Expect(2, 0), Expect(3, 0),
    Run(4_000_000), Expect(2, 1), Expect(3, 0),
    Run(8_000_000), Expect(2, 1), Expect(3, 1),
])]
// Dot-access read of Q returns TRUE after PT elapses.
#[case::dot_access_reads_q_after_pt(TON_DOT_READ_Q, &[
    Run(0), Expect(1, 0), Run(600_000), Expect(1, 1),
])]
// Dot-access read of Q stays FALSE before PT elapses.
#[case::dot_access_reads_q_before_pt(TON_DOT_READ_Q, &[
    Run(0), Run(100_000), Expect(1, 0),
])]
// Dot-access read of a non-BOOL field (ET) returns elapsed ms.
#[case::dot_access_reads_et(TON_DOT_READ_ET, &[
    Run(0), Run(3_000_000), Expect(1, 3000),
])]
// Dot-access writes of inputs + bare call + dot-access read of Q.
#[case::dot_access_writes_inputs(TON_DOT_WRITE_500MS, &[
    Run(0), Expect(1, 0), Run(600_000), Expect(1, 1),
])]
// Dot-access write of PT uses the new (longer) period.
#[case::dot_access_writes_pt(TON_DOT_WRITE_2S, &[
    Run(0), Run(1_000_000), Expect(1, 0), Run(3_000_000), Expect(1, 1),
])]
// Dot-access read of Q as an IF condition (issue #1375).
#[case::dot_access_reads_q_in_if(TON_DOT_READ_Q_IN_IF, &[
    Run(0), Run(100_000), Expect(1, 0), Run(600_000), Expect(1, 1),
])]
// Dot-access read of Q combined with other terms in a boolean expression.
#[case::dot_access_reads_q_in_expr(TON_DOT_READ_Q_IN_EXPR, &[
    // gate FALSE: result stays FALSE regardless of Q.
    Run(0), Expect(2, 0),
    // gate TRUE and Q still FALSE: result TRUE.
    Write(1, 1), Run(100_000), Expect(2, 1),
    // gate TRUE but Q now TRUE: result back to FALSE.
    Run(600_000), Expect(2, 0),
])]
// Dot-access read of the TIME output ET in a comparison.
#[case::dot_access_reads_et_in_if(TON_DOT_READ_ET_IN_IF, &[
    Run(0), Run(1_000_000), Expect(1, 0), Run(3_000_000), Expect(1, 1),
])]
// Dot-access read of Q as a WHILE loop guard.
#[case::dot_access_reads_q_in_while(TON_DOT_READ_Q_IN_WHILE, &[
    // Q still FALSE, so the loop runs to its count bound.
    Run(0), Run(100_000), Expect(1, 3),
])]
// A declaration member initializer sets PT: Q stays FALSE before it elapses
// and goes TRUE after, exactly as an explicit `PT := T#5s` argument would.
#[case::member_initializer_sets_pt(TON_MEMBER_INIT_PT, &[
    Run(0), Expect(1, 0), Run(2_000_000), Expect(1, 0), Run(6_000_000), Expect(1, 1),
])]
// An invocation argument overrides the declaration's member initializer.
#[case::member_initializer_overridden_by_invocation(TON_MEMBER_INIT_PT_OVERRIDDEN, &[
    Run(0), Expect(1, 0), Run(2_000_000), Expect(1, 1),
])]
fn end_to_end_fb_ton(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}

#[test]
fn end_to_end_when_time_variable_then_debug_type_name_is_time() {
    let source = "
PROGRAM main
  VAR
    elapsed : TIME;
  END_VAR
  elapsed := T#5s;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let elapsed_entry = debug
        .var_names
        .iter()
        .find(|e| e.name == "elapsed")
        .unwrap();
    assert_eq!(
        elapsed_entry.type_name, "TIME",
        "TIME variable should have type_name TIME, got {}",
        elapsed_entry.type_name
    );
}
