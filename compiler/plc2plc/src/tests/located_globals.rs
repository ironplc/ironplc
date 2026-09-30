//! Located variables (`name AT %QX0.0 : BOOL`) in `VAR_GLOBAL` blocks.
//!
//! These round-trip a top-level `VAR_GLOBAL` block
//! (`--allow-top-level-var-global`). It is parsed by the same
//! `global_var_decl` rule as the block of a `CONFIGURATION` or a `RESOURCE`,
//! and its declarations are rendered by the same `visit_var_decl`.
//!
//! Each block holds one declaration: the renderer writes a top-level block
//! as one block per declaration, which re-parses to a different number of
//! library elements whatever the declarations are.

use super::common::*;

fn top_level_var_global() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn write_to_string_when_global_located_then_round_trips() {
    let source = "
VAR_GLOBAL
    level AT %IW2 : INT := 7;
END_VAR
";
    assert_round_trips(source, &top_level_var_global());
}

#[test]
fn write_to_string_when_global_located_without_name_then_round_trips() {
    let source = "
VAR_GLOBAL
    AT %MW4 : INT;
END_VAR
";
    assert_round_trips(source, &top_level_var_global());
}

#[test]
fn write_to_string_when_global_block_retain_located_then_round_trips() {
    let source = "
VAR_GLOBAL RETAIN
    lamp AT %QX0.1 : BOOL;
END_VAR
";
    assert_round_trips(source, &top_level_var_global());
}
