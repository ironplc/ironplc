//! End-to-end tests for function block invocation (TP pulse timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TP function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, run, write, Duration, FbStep};

const TP_IN_TRUE: &str = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";

const TP_ET: &str = "
PROGRAM main
  VAR
    timer : TP;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s, ET => elapsed);
END_PROGRAM
";

const TP_ENABLE_Q_ET: &str = "
PROGRAM main
  VAR
    timer : TP;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";

const TP_TWO: &str = "
PROGRAM main
  VAR
    timer1 : TP;
    timer2 : TP;
    enable : BOOL;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := enable, PT := T#3s, Q => q1);
  timer2(IN := enable, PT := T#7s, Q => q2);
END_PROGRAM
";

#[rstest]
// Pulse starts on the rising edge: Q TRUE.
#[case::triggered(TP_IN_TRUE, &[run(0), expect("result", 1)])]
// Within PT the pulse stays TRUE.
#[case::before_pt(TP_IN_TRUE, &[run(0), run(2_000_000), expect("result", 1)])]
// Past PT the pulse ends: Q FALSE.
#[case::after_pt(TP_IN_TRUE, &[run(0), run(6_000_000), expect("result", 0)])]
// ET reports 3s of pulse elapsed.
#[case::reads_et(TP_ET, &[run(0), run(3_000_000), expect("elapsed", Duration::seconds(3))])]
// IN falling during the pulse does not cut it short; ET clamps to PT.
#[case::in_falls_during_pulse(TP_ENABLE_Q_ET, &[
    write("enable", 1), run(0), expect("result", 1),
    write("enable", 0), run(2_000_000), expect("result", 1),
    run(6_000_000), expect("result", 0), expect("elapsed", Duration::seconds(5)),
])]
// ET == PT exactly: pulse has ended, Q FALSE.
#[case::at_exact_pt(TP_IN_TRUE, &[run(0), run(5_000_000), expect("result", 0)])]
// Two TP timers with different PT run independently.
#[case::two_timers(TP_TWO, &[
    write("enable", 1), run(0), expect("q1", 1), expect("q2", 1),
    write("enable", 0),
    run(4_000_000), expect("q1", 0), expect("q2", 1),
    run(8_000_000), expect("q1", 0), expect("q2", 0),
])]
fn end_to_end_fb_tp(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
