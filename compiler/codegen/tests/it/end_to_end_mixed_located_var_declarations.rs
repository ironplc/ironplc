//! End-to-end integration tests for mixing `AT`-located variables with
//! plain variables in the same `VAR`/`VAR_INPUT`/`VAR_OUTPUT` block,
//! enabled by `--allow-mixed-located-var-declarations`.
//!
//! Codegen does not special-case `VariableIdentifier::Direct` at all (it's
//! allocated and read/written exactly like any other variable slot), so
//! these tests focus on proving the *plain* sibling in a mixed block still
//! compiles and runs correctly -- the located variable's own runtime
//! behavior is already covered by the pre-existing, dedicated-block tests.

use ironplc_parser::options::CompilerOptions;

// var layout: result=0, tempSensor=1 (plain variables are allocated
// before located ones, unrelated to source declaration order --
// confirmed via debug_section.var_names, same as other located-variable
// end-to-end tests elsewhere in this suite).
e2e_i32_with!(
    end_to_end_when_mixed_var_block_then_plain_variable_readable,
    CompilerOptions {
        allow_mixed_located_var_declarations: true,
        ..CompilerOptions::default()
    },
    "
PROGRAM main
VAR
    tempSensor AT%I*: INT;
    result : INT;
END_VAR
    result := 42;
END_PROGRAM
",
    &[(0, 42)],
);

// var layout: inst=0 (struct), out=1
e2e_i32_with!(
    end_to_end_when_mixed_var_input_block_then_plain_input_readable,
    CompilerOptions {
        allow_mixed_located_var_declarations: true,
        ..CompilerOptions::default()
    },
    "
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
",
    &[(1, 42)],
);
