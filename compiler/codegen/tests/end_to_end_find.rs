//! End-to-end integration tests for the FIND standard function.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

// FIND returns the 1-based position of the first occurrence of s2 in s1,
// or 0 if not found. Envelope:
//   VAR s1 : STRING := <s1_init>; s2 : STRING := <s2_init>; n : INT; END_VAR;
//   n := FIND(s1, s2);
// `n` sits at vars[2].
#[rstest]
#[case::substring_match("'Hello World'", "'World'", 7)]
#[case::not_found("'Hello World'", "'XYZ'", 0)]
#[case::at_start("'Hello World'", "'H'", 1)]
#[case::empty_search("'Hello'", "", 0)]
#[case::exact_match("'abc'", "'abc'", 1)]
#[case::search_longer_than_haystack("'Hi'", "'Hello'", 0)]
#[case::at_end("'ABCDE'", "'DE'", 4)]
fn end_to_end_find(#[case] s1_init: &str, #[case] s2_init: &str, #[case] expected: i32) {
    let s2_decl = if s2_init.is_empty() {
        "s2 : STRING;".to_string()
    } else {
        format!("s2 : STRING := {s2_init};")
    };
    let source = format!(
        "PROGRAM main VAR s1 : STRING := {s1_init}; {s2_decl} n : INT; END_VAR n := FIND(s1, s2); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), expected);
}

// FIND with a nested MID call: MID('world', 3, 1) = 'wor', FIND(..., 'wor') = 7.
e2e_i32!(
    end_to_end_when_find_with_nested_mid_then_returns_position,
    "PROGRAM main VAR s1 : STRING := 'hello world'; s2 : STRING := 'world'; n : INT; END_VAR n := FIND(s1, MID(s2, 3, 1)); END_PROGRAM",
    &[(2, 7)],
);

// FIND on a struct-array field; uses the Rusty dialect so system-uptime vars
// push the result to vars[4].
e2e_i32_with!(
    end_to_end_when_find_with_struct_array_field_then_returns_position,
    CompilerOptions::from_dialect(Dialect::Rusty),
    "TYPE MY_SETUP : STRUCT NAMES : ARRAY[1..3] OF STRING[20]; END_STRUCT; END_TYPE VAR_GLOBAL setup : MY_SETUP; END_VAR PROGRAM main VAR pos : INT; END_VAR setup.NAMES[1] := 'alpha'; setup.NAMES[2] := 'beta'; pos := FIND(setup.NAMES[2], 'bet'); END_PROGRAM",
    &[(4, 1)],
);
