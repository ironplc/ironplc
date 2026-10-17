//! End-to-end tests for the outputs of a function or method call: each is
//! stored in its target after the call, read from the callee's own output
//! variables and converted to the target's type.

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

/// A function `F` and a method `K.M`, each returning its input and with
/// outputs of several types.
const CALLEES: &str = "
FUNCTION F : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT q : REAL; u : UDINT; done : BOOL; n : DINT; END_VAR
  q := 1.5;
  u := 4000000000;
  done := x > 0;
  n := n + x;
  F := x;
END_FUNCTION
FUNCTION_BLOCK K
VAR count : DINT; END_VAR
METHOD M : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT mq : REAL; mn : DINT; END_VAR
  count := count + x;
  mq := 2.5;
  mn := count;
  M := x;
END_METHOD
END_FUNCTION_BLOCK
";

fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[spec_test(REQ_CB_codegen_060)]
fn end_to_end_when_function_outputs_assigned_then_stored_converted() {
    let source = format!(
        "{CALLEES}
PROGRAM main
VAR r : DINT; lr : LREAL; lu : LINT; nd : BOOL; END_VAR
  r := F(x := 3, q => lr, u => lu, NOT done => nd);
END_PROGRAM"
    );
    crate::common::assert_run_with::<i64>(
        &source,
        &options(),
        &[("r", 3), ("lu", 4_000_000_000), ("nd", 0)],
    );
    crate::common::assert_run_with::<f64>(&source, &options(), &[("lr", 1.5)]);
}

#[spec_test(REQ_CB_codegen_061)]
fn end_to_end_when_method_outputs_assigned_then_stored_converted() {
    let source = format!(
        "{CALLEES}
PROGRAM main
VAR k : K; mr : DINT; mlr : LREAL; a : ARRAY[1..2] OF LINT; a2 : LINT; END_VAR
  mr := k.M(x := 5, mq => mlr);
  k.M(x := 6, mn => a[2]);
  a2 := a[2];
END_PROGRAM"
    );
    crate::common::assert_run_with::<i64>(&source, &options(), &[("mr", 5), ("a2", 11)]);
    crate::common::assert_run_with::<f64>(&source, &options(), &[("mlr", 2.5)]);
}

#[spec_test(REQ_CB_codegen_062)]
fn end_to_end_when_function_called_twice_then_output_starts_again() {
    let source = format!(
        "{CALLEES}
PROGRAM main
VAR r : DINT; n1 : DINT; n2 : DINT; END_VAR
  r := F(x := 3, n => n1) + F(x := 4, n => n2);
END_PROGRAM"
    );
    crate::common::assert_run_with::<i32>(&source, &options(), &[("r", 7), ("n1", 3), ("n2", 4)]);
}
