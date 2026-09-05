//! End-to-end tests for function block invocation (TP pulse timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TP function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

// timer=var0, result=var1.
const TP_IN_TRUE: &str = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";

// timer=var0, elapsed=var1.
const TP_ET: &str = "
PROGRAM main
  VAR
    timer : TP;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s, ET => elapsed);
END_PROGRAM
";

// timer=var0, enable=var1, result=var2, elapsed=var3.
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

// timer1=var0, timer2=var1, enable=var2, q1=var3, q2=var4.
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
#[case::triggered(TP_IN_TRUE, &[Run(0), Expect(1, 1)])]
// Within PT the pulse stays TRUE.
#[case::before_pt(TP_IN_TRUE, &[Run(0), Run(2_000_000), Expect(1, 1)])]
// Past PT the pulse ends: Q FALSE.
#[case::after_pt(TP_IN_TRUE, &[Run(0), Run(6_000_000), Expect(1, 0)])]
// ET reports 3s of pulse elapsed.
#[case::reads_et(TP_ET, &[Run(0), Run(3_000_000), Expect(1, 3000)])]
// IN falling during the pulse does not cut it short; ET clamps to PT.
#[case::in_falls_during_pulse(TP_ENABLE_Q_ET, &[
    Write(1, 1), Run(0), Expect(2, 1),
    Write(1, 0), Run(2_000_000), Expect(2, 1),
    Run(6_000_000), Expect(2, 0), Expect(3, 5000),
])]
// ET == PT exactly: pulse has ended, Q FALSE.
#[case::at_exact_pt(TP_IN_TRUE, &[Run(0), Run(5_000_000), Expect(1, 0)])]
// Two TP timers with different PT run independently.
#[case::two_timers(TP_TWO, &[
    Write(2, 1), Run(0), Expect(3, 1), Expect(4, 1),
    Write(2, 0),
    Run(4_000_000), Expect(3, 0), Expect(4, 1),
    Run(8_000_000), Expect(3, 0), Expect(4, 0),
])]
fn end_to_end_fb_tp(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
