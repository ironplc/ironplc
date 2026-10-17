//! Tests that a whole structure output of a function block, which analysis
//! accepts as the source of an assignment to a structure of its type
//! (REQ-VB-analyzer-051, REQ-CB-analyzer-038), is reported rather than
//! compiled to a wrong value.
//!
//! Neither route copies one yet. The route that reads the AST copies a whole
//! structure only from a structure variable or a function's result, and
//! reports any other source as not implemented (`compile_aggregate.rs`).
//! Lowering makes the assignment a `Copy`, which the lowered route emits only
//! between the places its layout keeps a region for, so it leaves this one
//! to the AST route. A block whose body writes a field of the output does not
//! compile at all (`compile_struct.rs`).

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::try_parse_and_compile;

/// A block with a structure output `p`, whose body is `block_body`, called
/// by a program whose body is `body`.
fn program(block_body: &str, body: &str) -> String {
    format!(
        "TYPE P : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE
         FUNCTION_BLOCK FB
           VAR_INPUT x : DINT; END_VAR
           VAR_OUTPUT p : P; END_VAR
           VAR n : DINT; END_VAR
           {block_body}
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : FB; r : P; q : P; END_VAR
           {body}
         END_PROGRAM"
    )
}

#[rstest]
#[case::field_read("n := x;", "f(x := 1);\n r := f.p;")]
#[case::output("n := x;", "f(x := 1, p => q);")]
#[case::output_written_by_block("p.a := x;", "f(x := 1, p => q);")]
fn compile_when_block_structure_output_stored_whole_then_not_implemented(
    #[case] block_body: &str,
    #[case] body: &str,
) {
    let result = try_parse_and_compile(&program(block_body, body), &CompilerOptions::default());

    assert_eq!(result.unwrap_err().code, "P9999");
}
