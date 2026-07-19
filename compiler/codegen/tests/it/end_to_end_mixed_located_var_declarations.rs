//! End-to-end integration tests for mixing `AT`-located variables with
//! plain variables in the same `VAR`/`VAR_INPUT`/`VAR_OUTPUT` block,
//! enabled by `--allow-mixed-located-var-declarations`.
//!
//! See specs/plans/2026-07-19-twincat-mixed-located-var-declarations.md.
//!
//! Codegen does not special-case `VariableIdentifier::Direct` at all (it's
//! allocated and read/written exactly like any other variable slot), so
//! these tests focus on proving the *plain* sibling in a mixed block still
//! compiles and runs correctly -- the located variable's own runtime
//! behavior is already covered by the pre-existing, dedicated-block tests.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

#[test]
fn end_to_end_when_mixed_var_block_then_plain_variable_readable() {
    let source = "
PROGRAM main
VAR
    tempSensor AT%I*: INT;
    result : INT;
END_VAR
    result := 42;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_mixed_located_var_declarations: true,
        ..CompilerOptions::default()
    };
    let (_c, bufs) = parse_and_run(source, &options);

    // var layout: result=0, tempSensor=1 (plain variables are allocated
    // before located ones, unrelated to source declaration order --
    // confirmed via debug_section.var_names, same as other located-variable
    // end-to-end tests elsewhere in this suite).
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_mixed_var_input_block_then_plain_input_readable() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR_INPUT
    tempSensor AT%I*: INT;
    scale : INT;
END_VAR
VAR_OUTPUT
    result : INT;
END_VAR
    result := scale * 2;
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Example;
    out : INT;
END_VAR
    inst(scale := 21);
    out := inst.result;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_mixed_located_var_declarations: true,
        ..CompilerOptions::default()
    };
    let (_c, bufs) = parse_and_run(source, &options);

    // var layout: inst=0 (struct), out=1
    assert_eq!(bufs.vars[1].as_i32(), 42);
}
