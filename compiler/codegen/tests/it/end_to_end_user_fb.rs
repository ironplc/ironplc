//! End-to-end tests for user-defined function block compilation and execution.
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! user-defined FUNCTION_BLOCK declarations, compile to bytecode, and execute
//! on the VM.

use ironplc_parser::options::CompilerOptions;

use crate::common::run_scans;

// A user-defined function block instance declared with member initial values
// (`fb : SCALER := (factor := 3)`) is an instance, not a structure: the
// members are set once during setup and the block is invocable.
e2e_i32!(
    end_to_end_when_user_fb_declared_with_member_initializer_then_member_is_set,
    "
FUNCTION_BLOCK SCALER
  VAR_INPUT x : DINT; factor : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * factor;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : SCALER := (factor := 3);
    result : DINT;
  END_VAR
  fb(x := 7, y => result);
END_PROGRAM
",
    &[("result", 21)],
);

e2e_i32!(
    end_to_end_when_user_fb_simple_input_output_then_computes_result,
    "
FUNCTION_BLOCK DOUBLER
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : DOUBLER;
    result : DINT;
  END_VAR
  fb(x := 7, y => result);
END_PROGRAM
",
    &[("result", 14)],
);

#[test]
fn end_to_end_when_user_fb_internal_state_then_persists_across_calls() {
    let source = "
FUNCTION_BLOCK ACCUMULATOR
  VAR_INPUT val : DINT; END_VAR
  VAR total : DINT; END_VAR
  VAR_OUTPUT sum : DINT; END_VAR
  total := total + val;
  sum := total;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    acc : ACCUMULATOR;
    result : DINT;
  END_VAR
  acc(val := 10, sum => result);
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        // Round 1: total = 0 + 10 = 10
        session.scan(0).unwrap();
        assert_eq!(session.read("result"), 10, "round 1: sum should be 10");

        // Round 2: total = 10 + 10 = 20 (state persists in data region)
        session.scan(0).unwrap();
        assert_eq!(session.read("result"), 20, "round 2: sum should be 20");
    });
}

#[test]
fn end_to_end_when_two_user_fb_instances_then_independent_state() {
    let source = "
FUNCTION_BLOCK COUNTER
  VAR_INPUT inc : DINT; END_VAR
  VAR count : DINT; END_VAR
  VAR_OUTPUT value : DINT; END_VAR
  count := count + inc;
  value := count;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    c1 : COUNTER;
    c2 : COUNTER;
    r1 : DINT;
    r2 : DINT;
  END_VAR
  c1(inc := 1, value => r1);
  c2(inc := 100, value => r2);
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("r1"), 1, "c1 should be 1");
        assert_eq!(session.read("r2"), 100, "c2 should be 100");

        session.scan(0).unwrap();
        assert_eq!(session.read("r1"), 2, "c1 should be 2");
        assert_eq!(session.read("r2"), 200, "c2 should be 200");
    });
}

#[test]
fn end_to_end_when_user_fb_body_calls_user_function_then_compiles() {
    // Regression for codegen P9999: calling a user-defined FUNCTION from
    // inside a FUNCTION_BLOCK body previously failed in codegen because user
    // functions were registered into `ctx.user_functions` only after FB
    // bodies were compiled. Compiling FUNCTIONs before FB bodies fixes this.
    let source = "
FUNCTION ADD_ONE : INT
  VAR_INPUT x : INT; END_VAR
  ADD_ONE := x + 1;
END_FUNCTION

FUNCTION_BLOCK COUNTER
  VAR count : INT; END_VAR
  count := ADD_ONE(x := count);
END_FUNCTION_BLOCK

PROGRAM main
  VAR c : COUNTER; END_VAR
  c();
END_PROGRAM
";
    let _ = crate::common::try_parse_and_compile(source, &CompilerOptions::default()).unwrap();
}

// Mirror of the DOUBLER test using dot-access instead of `Q => result`.
e2e_i32!(
    end_to_end_when_user_fb_dot_access_read_then_returns_output,
    "
FUNCTION_BLOCK DOUBLER
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : DOUBLER;
    result : DINT;
  END_VAR
  fb(x := 7);
  result := fb.y;
END_PROGRAM
",
    &[("result", 14)],
);

#[test]
fn end_to_end_when_user_fb_dot_access_across_rounds_then_stack_stable() {
    // Proves SWAP+POP cleans the fb_ref between iterations: if the stack
    // drifted, repeated rounds would either trap or produce wrong values.
    let source = "
FUNCTION_BLOCK ACCUMULATOR
  VAR_INPUT val : DINT; END_VAR
  VAR total : DINT; END_VAR
  VAR_OUTPUT sum : DINT; END_VAR
  total := total + val;
  sum := total;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    acc : ACCUMULATOR;
    result : DINT;
  END_VAR
  acc(val := 10);
  result := acc.sum;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("result"), 10, "round 1: result should be 10");
        session.scan(0).unwrap();
        assert_eq!(session.read("result"), 20, "round 2: result should be 20");
        session.scan(0).unwrap();
        assert_eq!(session.read("result"), 30, "round 3: result should be 30");
    });
}

// Mirror of the DOUBLER test using dot-access write to set the input
// before the bare FB call.
e2e_i32!(
    end_to_end_when_user_fb_dot_access_write_then_input_set,
    "
FUNCTION_BLOCK DOUBLER
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : DOUBLER;
    result : DINT;
  END_VAR
  fb.x := 7;
  fb();
  result := fb.y;
END_PROGRAM
",
    &[("result", 14)],
);

/// A block of `fields` fields: `fields - 1` `DINT` inputs and the output
/// `q`, one more than the last input. The program calls two instances with
/// different last inputs and reads both outputs.
fn wide_block_called(fields: usize) -> String {
    let last = fields - 2;
    let others: String = (0..last).map(|i| format!("f{i} : DINT; ")).collect();
    format!(
        "FUNCTION_BLOCK Wide
           VAR_INPUT {others}f{last} : DINT; END_VAR
           VAR_OUTPUT q : DINT; END_VAR
           q := f{last} + 1;
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR w : Wide; v : Wide; r : DINT; s : DINT; END_VAR
           w(f{last} := 7);
           v(f{last} := 40);
           r := w.q;
           s := v.q;
         END_PROGRAM"
    )
}

/// REQ-LOW-codegen-191: a call of a block of 255 fields, as many as `FB_CALL`
/// counts, copies every field in and out.
#[test]
fn run_when_block_with_255_fields_called_then_every_field_copied() {
    let container = crate::common::parse_and_compile_to_run(
        &wide_block_called(255),
        &CompilerOptions::default(),
    );
    let snapshot = crate::common::Snapshot::run_compiled(container);
    assert_eq!(snapshot.read_as::<i32>("r"), 8);
    assert_eq!(snapshot.read_as::<i32>("s"), 41);
}

/// REQ-LOW-codegen-191: a block of more than 255 fields is P9997, a limit of
/// the bytecode format. Before, such a block compiled, and a call copied only
/// the low byte of its field count: 0 fields for 256.
#[test]
fn compile_when_block_with_more_than_255_fields_then_p9997() {
    for fields in [256, 300] {
        let result = crate::common::try_parse_and_compile(
            &wide_block_called(fields),
            &CompilerOptions::default(),
        );
        assert_eq!(
            result.err().map(|diagnostic| diagnostic.code),
            Some("P9997".to_string()),
            "{fields} fields"
        );
    }
}

/// REQ-LOW-codegen-191: a block's inputs, outputs and internal variables
/// all count toward the limit, and an instance is enough to report it, even
/// one the program never calls. (A block no program reaches is not compiled,
/// so it is not reported.)
#[test]
fn compile_when_block_with_256_fields_of_each_kind_declared_then_p9997() {
    let inputs: String = (0..100).map(|i| format!("a{i} : DINT; ")).collect();
    let outputs: String = (0..100).map(|i| format!("b{i} : DINT; ")).collect();
    let locals: String = (0..56).map(|i| format!("c{i} : DINT; ")).collect();
    let source = format!(
        "FUNCTION_BLOCK Wide
           VAR_INPUT {inputs}END_VAR
           VAR_OUTPUT {outputs}END_VAR
           VAR {locals}END_VAR
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR w : Wide; r : DINT; END_VAR
           r := 1;
         END_PROGRAM"
    );
    let result = crate::common::try_parse_and_compile(&source, &CompilerOptions::default());
    assert_eq!(
        result.err().map(|diagnostic| diagnostic.code),
        Some("P9997".to_string())
    );
}
