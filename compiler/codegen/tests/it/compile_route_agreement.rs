//! Tests of how the end-to-end helpers compare the two routes: by each
//! variable's name, after `init` and after one scan (REQ-LOW-codegen-131).
//!
//! Each test compiles two programs by the AST route and compares them as
//! the helpers compare a program compiled by both routes.

use ironplc_parser::options::CompilerOptions;

use crate::common::{assert_routes_agree, parse_and_compile};

fn compile(source: &str) -> ironplc_container::Container {
    parse_and_compile(source, &CompilerOptions::default())
}

#[test]
fn assert_routes_agree_when_same_variables_in_other_slots_then_agree() {
    let first = compile(
        "PROGRAM main VAR a : DINT := 1; b : LREAL := 2.5; END_VAR a := a + 1; END_PROGRAM",
    );
    let second = compile(
        "PROGRAM main VAR b : LREAL := 2.5; a : DINT := 1; END_VAR a := a + 1; END_PROGRAM",
    );

    assert_routes_agree(&first, &second);
}

#[test]
#[should_panic(expected = "after init, the AST and lowered routes leave variable a")]
fn assert_routes_agree_when_initial_values_differ_then_names_variable_after_init() {
    let first = compile("PROGRAM main VAR a : DINT := 1; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR a : DINT := 2; END_VAR a := 5; END_PROGRAM");

    assert_routes_agree(&first, &second);
}

#[test]
#[should_panic(expected = "after one scan, the AST and lowered routes leave variable a")]
fn assert_routes_agree_when_scan_results_differ_then_names_variable_after_scan() {
    let first = compile("PROGRAM main VAR a : DINT; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR a : DINT; END_VAR a := 6; END_PROGRAM");

    assert_routes_agree(&first, &second);
}

#[test]
#[should_panic(expected = "after init, the AST and lowered routes leave variable s")]
fn assert_routes_agree_when_strings_differ_then_names_string_before_data_region() {
    let first = compile("PROGRAM main VAR s : STRING := 'ab'; END_VAR END_PROGRAM");
    let second = compile("PROGRAM main VAR s : STRING := 'ac'; END_VAR END_PROGRAM");

    assert_routes_agree(&first, &second);
}

#[test]
#[should_panic(expected = "only one route names the variable a")]
fn assert_routes_agree_when_one_names_another_variable_then_names_it() {
    let first = compile("PROGRAM main VAR a : DINT; END_VAR a := 5; END_PROGRAM");
    let second = compile("PROGRAM main VAR b : DINT; END_VAR b := 5; END_PROGRAM");

    assert_routes_agree(&first, &second);
}
