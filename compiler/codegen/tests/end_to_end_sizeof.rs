//! End-to-end integration tests for the SIZEOF operator.

mod common;
use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

fn sizeof_options() -> CompilerOptions {
    CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    }
}

// Envelope: VAR x : <ty>; s : DINT; END_VAR; s := SIZEOF(x);
// `s` lives at vars[1] and holds the size in bytes.
#[rstest]
#[case::int(2, "INT", 2)]
#[case::dint(1, "DINT", 4)]
#[case::dword(1, "DWORD", 4)]
#[case::bool_(1, "BOOL", 1)]
#[case::real(1, "REAL", 4)]
#[case::lreal(1, "LREAL", 8)]
#[case::array_10_of_int(1, "ARRAY[1..10] OF INT", 20)]
fn end_to_end_sizeof(#[case] _ordinal: u8, #[case] ty: &str, #[case] expected_bytes: i32) {
    let source =
        format!("PROGRAM main VAR x : {ty}; s : DINT; END_VAR s := SIZEOF(x); END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &sizeof_options());
    assert_eq!(bufs.vars[1].as_i32(), expected_bytes);
}
