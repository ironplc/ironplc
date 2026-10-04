//! End-to-end tests for function block invocation (TOF off-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TOF function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, run, write, Duration, FbStep};

const TOF_IN_TRUE: &str = "
PROGRAM main
  VAR
    timer : TOF;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";

const TOF_ENABLE: &str = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result);
END_PROGRAM
";

const TOF_ENABLE_ET: &str = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#10s, ET => elapsed);
END_PROGRAM
";

const TOF_ENABLE_Q_ET: &str = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";

const TOF_TWO: &str = "
PROGRAM main
  VAR
    timer1 : TOF;
    timer2 : TOF;
    enable : BOOL;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := enable, PT := T#3s, Q => q1);
  timer2(IN := enable, PT := T#7s, Q => q2);
END_PROGRAM
";

#[rstest]
// IN TRUE: Q is TRUE immediately.
#[case::in_true(TOF_IN_TRUE, &[run(0), expect("result", 1)])]
// After the falling edge, Q stays TRUE while still within PT.
#[case::in_false_before_pt(TOF_ENABLE, &[
    write("enable", 1), run(0), expect("result", 1),
    write("enable", 0), run(1_000_000),
    run(3_000_000), expect("result", 1),
])]
// Past PT after the falling edge: Q goes FALSE.
#[case::in_false_after_pt(TOF_ENABLE, &[
    write("enable", 1), run(0),
    write("enable", 0), run(1_000_000),
    run(7_000_000), expect("result", 0),
])]
// ET reports 3s of off-delay elapsed.
#[case::reads_et(TOF_ENABLE_ET, &[
    write("enable", 1), run(0),
    write("enable", 0), run(1_000_000),
    run(4_000_000), expect("elapsed", Duration::seconds(3)),
])]
// IN rising during timing resets; a new falling edge restarts the delay.
#[case::in_rises_resets(TOF_ENABLE_Q_ET, &[
    write("enable", 1), run(0), expect("result", 1),
    write("enable", 0), run(1_000_000),
    run(3_000_000), expect("result", 1),
    write("enable", 1), run(4_000_000), expect("result", 1), expect("elapsed", Duration::ZERO),
    write("enable", 0), run(5_000_000),
    run(8_000_000), expect("result", 1),
    run(11_000_000), expect("result", 0),
])]
// ET == PT exactly: Q is FALSE.
#[case::at_exact_pt(TOF_ENABLE, &[
    write("enable", 1), run(0),
    write("enable", 0), run(1_000_000),
    run(6_000_000), expect("result", 0),
])]
// Two TOF timers with different PT run independently.
#[case::two_timers(TOF_TWO, &[
    write("enable", 1), run(0), expect("q1", 1), expect("q2", 1),
    write("enable", 0), run(1_000_000),
    run(5_000_000), expect("q1", 0), expect("q2", 1),
    run(9_000_000), expect("q1", 0), expect("q2", 0),
])]
fn end_to_end_fb_tof(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
