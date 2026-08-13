//! End-to-end tests for function block invocation (TOF off-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TOF function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

// timer=var0, result=var1.
const TOF_IN_TRUE: &str = "
PROGRAM main
  VAR
    timer : TOF;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";

// timer=var0, enable=var1, result=var2.
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

// timer=var0, enable=var1, elapsed=var2.
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

// timer=var0, enable=var1, result=var2, elapsed=var3.
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

// timer1=var0, timer2=var1, enable=var2, q1=var3, q2=var4.
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
#[case::in_true(TOF_IN_TRUE, &[Run(0), Expect(1, 1)])]
// After the falling edge, Q stays TRUE while still within PT.
#[case::in_false_before_pt(TOF_ENABLE, &[
    Write(1, 1), Run(0), Expect(2, 1),
    Write(1, 0), Run(1_000_000),
    Run(3_000_000), Expect(2, 1),
])]
// Past PT after the falling edge: Q goes FALSE.
#[case::in_false_after_pt(TOF_ENABLE, &[
    Write(1, 1), Run(0),
    Write(1, 0), Run(1_000_000),
    Run(7_000_000), Expect(2, 0),
])]
// ET reports 3s of off-delay elapsed.
#[case::reads_et(TOF_ENABLE_ET, &[
    Write(1, 1), Run(0),
    Write(1, 0), Run(1_000_000),
    Run(4_000_000), Expect(2, 3000),
])]
// IN rising during timing resets; a new falling edge restarts the delay.
#[case::in_rises_resets(TOF_ENABLE_Q_ET, &[
    Write(1, 1), Run(0), Expect(2, 1),
    Write(1, 0), Run(1_000_000),
    Run(3_000_000), Expect(2, 1),
    Write(1, 1), Run(4_000_000), Expect(2, 1), Expect(3, 0),
    Write(1, 0), Run(5_000_000),
    Run(8_000_000), Expect(2, 1),
    Run(11_000_000), Expect(2, 0),
])]
// ET == PT exactly: Q is FALSE.
#[case::at_exact_pt(TOF_ENABLE, &[
    Write(1, 1), Run(0),
    Write(1, 0), Run(1_000_000),
    Run(6_000_000), Expect(2, 0),
])]
// Two TOF timers with different PT run independently.
#[case::two_timers(TOF_TWO, &[
    Write(2, 1), Run(0), Expect(3, 1), Expect(4, 1),
    Write(2, 0), Run(1_000_000),
    Run(5_000_000), Expect(3, 0), Expect(4, 1),
    Run(9_000_000), Expect(3, 0), Expect(4, 0),
])]
fn end_to_end_fb_tof(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
