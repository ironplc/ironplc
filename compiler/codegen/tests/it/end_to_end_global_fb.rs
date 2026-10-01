//! End-to-end integration tests for calling a global function block
//! instance: a `VAR_GLOBAL` instance reached through `VAR_EXTERNAL`, or a
//! top-level `VAR_GLOBAL` instance called directly.
//!
//! Every call reaches the one global instance, so its state persists across
//! scans and its outputs read through the name that called it.
//!
//! The configurations run their program every 10 ms, so their scan rounds
//! are 10 000 us apart.

use ironplc_parser::options::CompilerOptions;

use crate::common::{drive_fb, FbStep::*};

fn top_level_global_options() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    }
}

// g=var0 (global), q=var1, et=var2.
const TON_THROUGH_EXTERNAL: &str = "
PROGRAM main
  VAR_EXTERNAL
    g : TON;
  END_VAR
  VAR
    q : BOOL;
    et : TIME;
  END_VAR
  g(IN := TRUE, PT := T#1s);
  q := g.Q;
  et := g.ET;
END_PROGRAM

CONFIGURATION config
  VAR_GLOBAL
    g : TON;
  END_VAR
  RESOURCE res ON PLC
    TASK t (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM p WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

#[test]
fn end_to_end_when_configuration_ton_called_through_external_then_times_across_scans() {
    drive_fb(
        TON_THROUGH_EXTERNAL,
        &CompilerOptions::default(),
        &[
            Run(0),
            Expect(1, 0),
            Run(400_000),
            Expect(1, 0),
            Expect(2, 400),
            Run(1_000_000),
            Expect(1, 1),
            Expect(2, 1000),
        ],
    );
}

const COUNTER: &str = "
FUNCTION_BLOCK Counter
  VAR_INPUT
    inc : DINT;
  END_VAR
  VAR_OUTPUT
    total : DINT;
  END_VAR
  total := total + inc;
END_FUNCTION_BLOCK
";

// c=var0 (global), result=var1, via_output=var2.
const COUNTER_THROUGH_EXTERNAL: &str = "
PROGRAM main
  VAR_EXTERNAL
    c : Counter;
  END_VAR
  VAR
    result : DINT;
    via_output : DINT;
  END_VAR
  c(inc := 5, total => via_output);
  result := c.total;
END_PROGRAM

CONFIGURATION config
  VAR_GLOBAL
    c : Counter;
  END_VAR
  RESOURCE res ON PLC
    TASK t (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM p WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

#[test]
fn end_to_end_when_configuration_user_fb_called_through_external_then_state_persists() {
    drive_fb(
        &format!("{COUNTER}{COUNTER_THROUGH_EXTERNAL}"),
        &CompilerOptions::default(),
        &[
            Run(0),
            Expect(1, 5),
            Expect(2, 5),
            Run(10_000),
            Run(20_000),
            Expect(1, 15),
            Expect(2, 15),
        ],
    );
}

// c=var0 (global), stepper=var1, result=var2. The program and the body of
// Stepper both call the global Counter: 10 + 1 a scan.
const COUNTER_SHARED_WITH_FB_BODY: &str = "
FUNCTION_BLOCK Stepper
  VAR_EXTERNAL
    c : Counter;
  END_VAR
  c(inc := 1);
END_FUNCTION_BLOCK

PROGRAM main
  VAR_EXTERNAL
    c : Counter;
  END_VAR
  VAR
    stepper : Stepper;
    result : DINT;
  END_VAR
  c(inc := 10);
  stepper();
  result := c.total;
END_PROGRAM

CONFIGURATION config
  VAR_GLOBAL
    c : Counter;
  END_VAR
  RESOURCE res ON PLC
    TASK t (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM p WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

#[test]
fn end_to_end_when_fb_body_calls_global_through_external_then_shares_instance_with_program() {
    drive_fb(
        &format!("{COUNTER}{COUNTER_SHARED_WITH_FB_BODY}"),
        &CompilerOptions::default(),
        &[Run(0), Expect(2, 11), Run(10_000), Expect(2, 22)],
    );
}

// c=var0 (global), result=var1.
const COUNTER_TOP_LEVEL_THROUGH_EXTERNAL: &str = "
VAR_GLOBAL
  c : Counter;
END_VAR

PROGRAM main
  VAR_EXTERNAL
    c : Counter;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  c(inc := 5);
  result := c.total;
END_PROGRAM
";

#[test]
fn end_to_end_when_top_level_user_fb_called_through_external_then_state_persists() {
    drive_fb(
        &format!("{COUNTER}{COUNTER_TOP_LEVEL_THROUGH_EXTERNAL}"),
        &top_level_global_options(),
        &[Run(0), Expect(1, 5), Run(1), Expect(1, 10)],
    );
}

// c=var0, g=var1 (globals), result=var2, q=var3.
const TOP_LEVEL_CALLED_DIRECTLY: &str = "
VAR_GLOBAL
  c : Counter;
  g : TON;
END_VAR

PROGRAM main
  VAR
    result : DINT;
    q : BOOL;
  END_VAR
  c(inc := 5);
  result := c.total;
  g(IN := TRUE, PT := T#1s);
  q := g.Q;
END_PROGRAM
";

#[test]
fn end_to_end_when_top_level_globals_called_directly_then_state_persists() {
    drive_fb(
        &format!("{COUNTER}{TOP_LEVEL_CALLED_DIRECTLY}"),
        &top_level_global_options(),
        &[
            Run(0),
            Expect(2, 5),
            Expect(3, 0),
            Run(1_000_000),
            Expect(2, 10),
            Expect(3, 1),
        ],
    );
}

// m=var0 (global), result=var1.
const METHOD_THROUGH_EXTERNAL: &str = "
FUNCTION_BLOCK Accumulator
  VAR
    total : DINT;
  END_VAR
  METHOD Add
    VAR_INPUT
      amount : DINT;
    END_VAR
    total := total + amount;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR_EXTERNAL
    m : Accumulator;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  m.Add(amount := 3);
  result := m.total;
END_PROGRAM

CONFIGURATION config
  VAR_GLOBAL
    m : Accumulator;
  END_VAR
  RESOURCE res ON PLC
    TASK t (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM p WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

#[test]
fn end_to_end_when_method_called_through_external_then_state_persists() {
    drive_fb(
        METHOD_THROUGH_EXTERNAL,
        &CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        },
        &[Run(0), Expect(1, 3), Run(10_000), Expect(1, 6)],
    );
}
