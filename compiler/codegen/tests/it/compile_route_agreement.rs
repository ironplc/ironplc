//! Tests of how the end-to-end helpers compare the backends: by each
//! variable's name, after `init` and after one scan (REQ-LOW-codegen-131).
//!
//! Each test compiles two programs on the reference backend and compares
//! them as the helpers compare a program compiled on every backend.

use ironplc_parser::options::CompilerOptions;

use crate::common::{assert_backends_agree, parse_and_compile, Reference};

fn compile(source: &str) -> ironplc_container::Container {
    parse_and_compile(source, &CompilerOptions::default())
}

#[test]
fn assert_backends_agree_when_same_variables_in_other_slots_then_agree() {
    let first = compile(
        "PROGRAM main VAR a : DINT := 1; b : LREAL := 2.5; END_VAR a := a + 1; END_PROGRAM",
    );
    let second = compile(
        "PROGRAM main VAR b : LREAL := 2.5; a : DINT := 1; END_VAR a := a + 1; END_PROGRAM",
    );

    assert_backends_agree::<Reference, Reference>(&first, &second);
}

#[test]
#[should_panic(expected = "after init, variable a differs")]
fn assert_backends_agree_when_initial_values_differ_then_names_variable_after_init() {
    let first = compile("PROGRAM main VAR a : DINT := 1; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR a : DINT := 2; END_VAR a := 5; END_PROGRAM");

    assert_backends_agree::<Reference, Reference>(&first, &second);
}

#[test]
#[should_panic(expected = "after one scan, variable a differs")]
fn assert_backends_agree_when_scan_results_differ_then_names_variable_after_scan() {
    let first = compile("PROGRAM main VAR a : DINT; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR a : DINT; END_VAR a := 6; END_PROGRAM");

    assert_backends_agree::<Reference, Reference>(&first, &second);
}

#[test]
#[should_panic(expected = "after init, variable s differs")]
fn assert_backends_agree_when_strings_differ_then_names_string() {
    let first = compile("PROGRAM main VAR s : STRING := 'ab'; END_VAR END_PROGRAM");
    let second = compile("PROGRAM main VAR s : STRING := 'ac'; END_VAR END_PROGRAM");

    assert_backends_agree::<Reference, Reference>(&first, &second);
}

#[test]
#[should_panic(expected = "after init, only the bytecode backend names the variable a")]
fn assert_backends_agree_when_one_names_another_variable_then_names_it() {
    let first = compile("PROGRAM main VAR a : DINT; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR b : DINT; END_VAR b := 5; END_PROGRAM");

    assert_backends_agree::<Reference, Reference>(&first, &second);
}
