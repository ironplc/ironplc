//! End-to-end tests for method calls in expression position (OOP extension,
//! ADR-0041 Phase 1 static dispatch). The call leaves the instance reference
//! beneath the return value; the expression form drops the reference and
//! keeps the value (`SWAP; POP`).

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

fn opts_with_fb_inheritance() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// A motor with one method of each shape the tests need. Every call to
/// `GetSpeed` counts itself in `calls`, so a test can see how often it ran.
const FB_MOTOR: &str = "
FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
    calls : INT;
END_VAR
METHOD GetSpeed : REAL
    calls := calls + 1;
    GetSpeed := speed;
END_METHOD
METHOD Scaled : REAL
VAR_INPUT
    factor : REAL;
END_VAR
    Scaled := speed * factor;
END_METHOD
METHOD IsFast : BOOL
    IsFast := speed > 10.0;
END_METHOD
METHOD Level : INT
    Level := 7;
END_METHOD
METHOD SetSpeed
VAR_INPUT
    value : REAL;
END_VAR
    speed := value;
END_METHOD
END_FUNCTION_BLOCK
";

fn run_main(main_vars: &str, main_body: &str) -> Snapshot {
    let source = format!(
        "{FB_MOTOR}
PROGRAM main
VAR
    m : FB_Motor;
{main_vars}
END_VAR
m.SetSpeed(12.5);
{main_body}
END_PROGRAM
"
    );
    Snapshot::run(&source, &opts_with_fb_inheritance())
}

#[test]
fn end_to_end_when_method_call_assigned_then_variable_holds_return_value() {
    let snapshot = run_main("v : REAL;", "v := m.GetSpeed();");
    assert_eq!(snapshot.read_as::<f32>("v"), 12.5);
}

#[test]
fn end_to_end_when_method_call_in_arithmetic_with_argument_then_computes() {
    let snapshot = run_main("v : REAL;", "v := 1.0 + m.Scaled(factor := 2.0);");
    assert_eq!(snapshot.read_as::<f32>("v"), 26.0);
}

#[test]
fn end_to_end_when_method_call_is_if_condition_then_branch_taken() {
    let snapshot = run_main(
        "fast : BOOL; slow : BOOL;",
        "IF m.IsFast() THEN fast := TRUE; ELSE slow := TRUE; END_IF;",
    );
    assert_eq!(snapshot.read_as::<i32>("fast"), 1);
    assert_eq!(snapshot.read_as::<i32>("slow"), 0);
}

#[test]
fn end_to_end_when_method_call_is_argument_of_method_call_then_value_passed() {
    let snapshot = run_main("v : REAL;", "v := m.Scaled(m.Scaled(2.0));");
    // 12.5 * (12.5 * 2.0)
    assert_eq!(snapshot.read_as::<f32>("v"), 312.5);
}

#[test]
fn end_to_end_when_int_method_result_assigned_to_dint_then_widened() {
    let snapshot = run_main("wide : DINT;", "wide := m.Level();");
    assert_eq!(snapshot.read_as::<i32>("wide"), 7);
}

/// The call runs exactly once per evaluation, and what it writes to the
/// instance persists.
#[test]
fn end_to_end_when_method_with_side_effect_called_in_expression_then_runs_once() {
    let snapshot = run_main("v : REAL; n : INT;", "v := m.GetSpeed(); n := m.calls;");
    assert_eq!(snapshot.read_as::<f32>("v"), 12.5);
    assert_eq!(snapshot.read_as::<i32>("n"), 1);
}
