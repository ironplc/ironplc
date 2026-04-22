//! End-to-end tests for integer narrowing type conversions.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Each case declares `x : <from>; y : <to>;`, sets `x := <init>;`, then
// calls `<from>_TO_<to>(x)` into `y`. vars[1] holds the narrowed value.
#[rstest]
#[case::dint_to_int("DINT", "INT", 1000, 1000)]
#[case::lint_to_dint("LINT", "DINT", 42, 42)]
// 300 mod 256 = 44 (wrapping to i8 range).
#[case::dint_to_sint_overflow_wraps("DINT", "SINT", 300, 44)]
#[case::lint_to_sint("LINT", "SINT", 50, 50)]
#[case::ulint_to_udint("ULINT", "UDINT", 1000, 1000)]
fn end_to_end_narrowing(
    #[case] from: &str,
    #[case] to: &str,
    #[case] init: i64,
    #[case] expected: i32,
) {
    let source = format!(
        "PROGRAM main VAR x : {from}; y : {to}; END_VAR x := {init}; y := {from}_TO_{to}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    // SINT is 8-bit signed; mask to i8 range via the cast.
    if to == "SINT" {
        assert_eq!(bufs.vars[1].as_i32() as i8 as i32, expected);
    } else if to == "UDINT" {
        assert_eq!(bufs.vars[1].as_i32() as u32 as i32, expected);
    } else {
        assert_eq!(bufs.vars[1].as_i32(), expected);
    }
}
