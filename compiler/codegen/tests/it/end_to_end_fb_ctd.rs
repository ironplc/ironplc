//! End-to-end tests for function block invocation (CTD count down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTD function block instance, compile to bytecode, and execute on the VM.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, FbStep, FbStep::*};

const CTD_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTD;
    pulse : BOOL;
    load : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CD := pulse, LD := load, PV := 3, Q => result, CV => count);
END_PROGRAM
";

const CTD_DINT_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTD_DINT;
    result : BOOL;
    count : DINT;
  END_VAR
  counter(CD := FALSE, LD := TRUE, PV := 10, Q => result, CV => count);
END_PROGRAM
";

#[rstest]
// CV starts at 0, Q = (CV <= 0) is TRUE.
#[case::not_loaded(CTD_PROGRAM, &[Run(0), Expect(3, 1)])]
// LD loads CV=PV=3; Q FALSE because CV > 0.
#[case::loaded(CTD_PROGRAM, &[
    Write(2, 1), Run(0), Expect(4, 3), Expect(3, 0),
])]
// Load then count down 3 times to CV=0: Q TRUE.
#[case::counts_to_zero(CTD_PROGRAM, &[
    Write(2, 1), Run(0), Write(2, 0), Run(1), Expect(4, 3),
    Pulse { var: 1, n: 3, time_base: 2 },
    Expect(4, 0), Expect(3, 1),
])]
// Load then count down once to CV=2: Q FALSE.
#[case::above_zero(CTD_PROGRAM, &[
    Write(2, 1), Run(0), Write(2, 0), Run(1),
    Write(1, 1), Run(2), Expect(3, 0), Expect(4, 2),
])]
// CTD_DINT variant compiles and runs; load sets CV=10.
#[case::dint_variant(CTD_DINT_PROGRAM, &[Run(0), Expect(2, 10)])]
fn end_to_end_fb_ctd(#[case] source: &str, #[case] steps: &[FbStep]) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
