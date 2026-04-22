//! End-to-end integration tests for CASE statement compilation.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Envelope: VAR x : DINT; y : DINT; END_VAR; x := <x>; CASE x OF <arms> END_CASE;
// Each test verifies both x (vars[0]) and y (vars[1]).
#[rstest]
#[case::matches_first_arm(1, "1: y := 10; 2: y := 20;", 10)]
#[case::matches_second_arm(2, "1: y := 10; 2: y := 20;", 20)]
#[case::no_match_no_else_y_untouched(99, "1: y := 10; 2: y := 20;", 0)]
#[case::no_match_with_else(99, "1: y := 10; 2: y := 20; ELSE y := 99;", 99)]
#[case::multi_selector_matches_any(3, "1: y := 10; 2, 3: y := 30;", 30)]
#[case::subrange_matches_in_range(3, "1..5: y := 50; 10: y := 100;", 50)]
fn end_to_end_case(#[case] x: i32, #[case] arms: &str, #[case] expected_y: i32) {
    let source = format!(
        "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := {x}; CASE x OF {arms} END_CASE; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}
