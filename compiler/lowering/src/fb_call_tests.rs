//! Tests of lowering calls of function blocks (REQ-LOW-lowering-069).

use crate::test_support::lower;

/// A block with two inputs and outputs of several types.
const BLOCK: &str = "
FUNCTION_BLOCK FB
VAR_INPUT x : DINT; s : SINT; END_VAR
VAR_OUTPUT q : DINT; done : BOOL; r : REAL; END_VAR
  q := x + s;
  done := q > 0;
  r := 1.5;
END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-069: the inputs are assigned to the instance's fields in
/// the order the block declares them, whatever order the call names them
/// in, then the block runs, then each output is assigned from its field: a
/// negated output through `NOT`, and one stored in a wider type through the
/// conversion the analyzer recorded.
#[test]
fn lower_statement_when_block_called_then_inputs_call_and_outputs() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             PROGRAM main VAR f : FB; a : DINT; b : BOOL; l : LREAL; END_VAR
               f(s := 3, x := a, q => a, NOT done => b, r => l);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [[
            "assign v0^.f0.f0 (read i32s v0^.f1)",
            "assign v0^.f0.f1 (const i32s 3)",
            "fb-call v0^.f0 pou1",
            "assign v0^.f1 (read i32s v0^.f0.f2)",
            "assign v0^.f2 (bool_not bool (read bool v0^.f0.f3))",
            "assign v0^.f3 (convert f64 (read f32 v0^.f0.f4))",
        ]
        .join("\n")]
    );
}

/// REQ-LOW-lowering-069: an input the call omits is not assigned, so it
/// keeps its value, and a call with no outputs ends with the call.
#[test]
fn lower_statement_when_input_omitted_then_not_assigned() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             PROGRAM main VAR f : FB; END_VAR
               f(x := 1);
               f();
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0.f0 (const i32s 1)\nfb-call v0^.f0 pou1",
            "fb-call v0^.f0 pou1",
        ]
    );
}

/// A call inside another statement is among that statement's statements.
#[test]
fn lower_statement_when_block_called_inside_if_then_among_its_statements() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             PROGRAM main VAR f : FB; on : BOOL; n : DINT; END_VAR
               IF on THEN
                 f(x := n, q => n);
               END_IF;
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [[
            "if (read bool v0^.f1)",
            "  assign v0^.f0.f0 (read i32s v0^.f2)",
            "  fb-call v0^.f0 pou1",
            "  assign v0^.f2 (read i32s v0^.f0.f2)",
            "end",
        ]
        .join("\n")]
    );
}

/// A call that binds a `VAR_IN_OUT` is not lowered.
#[test]
fn lower_statement_when_in_out_bound_then_unsupported() {
    let outcome = lower(
        "FUNCTION_BLOCK Acc
         VAR_IN_OUT total : DINT; END_VAR
           total := total + 1;
         END_FUNCTION_BLOCK
         PROGRAM main VAR c : Acc; n : DINT; END_VAR
           c(total := n);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}

/// REQ-LOW-lowering-069: a call of a standard timer lowers as a call of any
/// other block: its inputs stored in the instance's fields, the call of the
/// POU named after the block, then its outputs read from the fields.
#[test]
fn lower_statement_when_timer_called_then_inputs_call_and_outputs() {
    let outcome = lower(
        "PROGRAM main VAR t : TON; run : BOOL; done : BOOL; elapsed : TIME; END_VAR
           t(IN := run, PT := T#5S, Q => done, ET => elapsed);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [[
            "assign v0^.f0.f0 (read bool v0^.f1)",
            "assign v0^.f0.f1 (const i32s 5000)",
            "fb-call v0^.f0 pou1",
            "assign v0^.f2 (read bool v0^.f0.f2)",
            "assign v0^.f3 (read i32s v0^.f0.f3)",
        ]
        .join("\n")]
    );
}

/// REQ-LOW-lowering-069: a call of a standard counter stores each input it
/// supplies, an input stored narrower than it is computed narrowed as any
/// store is (REQ-LOW-lowering-046), and leaves an omitted input as it was.
#[test]
fn lower_statement_when_counter_called_then_inputs_call_and_outputs() {
    let outcome = lower(
        "PROGRAM main
           VAR c : CTU; d : CTD_LINT; a : INT; b : INT; q : BOOL; n : INT; m : LINT; END_VAR
           c(CU := q, PV := a + b, Q => q, CV => n);
           d(CD := TRUE, PV := m, CV => m);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            [
                "assign v0^.f0.f0 (read bool v0^.f4)",
                "assign v0^.f0.f2 (truncate 16 (add i32s (read i32s v0^.f2) (read i32s v0^.f3)))",
                "fb-call v0^.f0 pou1",
                "assign v0^.f4 (read bool v0^.f0.f3)",
                "assign v0^.f5 (read i32s v0^.f0.f4)",
            ]
            .join("\n"),
            [
                "assign v0^.f1.f0 (const bool true)",
                "assign v0^.f1.f2 (read i64s v0^.f6)",
                "fb-call v0^.f1 pou2",
                "assign v0^.f6 (read i64s v0^.f1.f4)",
            ]
            .join("\n"),
        ]
    );
}

/// A call in a block's body of an instance that is a field of the block
/// runs the field, reached through the body's instance parameter.
#[test]
fn lower_statement_when_block_instance_is_field_of_block_then_field_called() {
    let outcome = lower(
        &format!(
            "{BLOCK}
             FUNCTION_BLOCK Outer
             VAR inner : FB; n : DINT; END_VAR
               inner(x := n);
             END_FUNCTION_BLOCK
             PROGRAM main VAR o : Outer; END_VAR
               o();
             END_PROGRAM"
        ),
        "Outer",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0.f0 (read i32s v0^.f1)\nfb-call v0^.f0 pou1"]
    );
}

/// REQ-LOW-lowering-069: a string output is assigned to its target as a
/// string is, from the instance's field.
#[test]
fn lower_statement_when_block_has_string_output_then_assigned_as_string() {
    let outcome = lower(
        "FUNCTION_BLOCK Named
           VAR_INPUT n : DINT; END_VAR
           VAR_OUTPUT s : STRING[8]; w : WSTRING; END_VAR
           s := 'a';
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : Named; t : STRING[4]; u : WSTRING; END_VAR
           f(n := 1, s => t, w => u);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [[
            "assign v0^.f0.f0 (const i32s 1)",
            "fb-call v0^.f0 pou1",
            "assign-str v0^.f1 (read string[8] v0^.f0.f1)",
            "assign-str v0^.f2 (read wstring[254] v0^.f0.f2)",
        ]
        .join("\n")]
    );
}
