//! End-to-end integration tests for a `VAR` initializer that is a bare
//! constant name (`x : UDINT := C;`), a constant expression enabled by
//! `--allow-constant-initializer-expressions`.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{assert_run_f64_with, assert_run_i32_with, assert_run_i64_with};

/// The default dialect with the flags the sources below need.
fn default_with_flag() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    }
}

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_initializer_names_global_constant_then_starts_with_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
PROGRAM main
VAR
    x : UDINT := C;
END_VAR
END_PROGRAM
";
    // var layout: C=0, x=1
    assert_run_i32_with(source, &options, &[(1, 7)]);
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_initializer_names_local_constant_then_starts_with_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
PROGRAM main
VAR CONSTANT
    L : DINT := -3;
END_VAR
VAR
    x : DINT := L;
END_VAR
END_PROGRAM
";
    // var layout: L=0, x=1
    assert_run_i32_with(source, &options, &[(1, -3)]);
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_constant_initialized_from_constant_then_both_hold_value(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    C : DINT := 7;
    D : DINT := C;
END_VAR
PROGRAM main
VAR
    x : DINT := D;
END_VAR
END_PROGRAM
";
    // var layout: C=0, D=1, x=2
    assert_run_i32_with(source, &options, &[(1, 7), (2, 7)]);
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_alias_initialized_from_constant_then_starts_with_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
TYPE MyInt : DINT; END_TYPE
VAR_GLOBAL CONSTANT
    C : DINT := 42;
END_VAR
PROGRAM main
VAR
    x : MyInt := C;
END_VAR
END_PROGRAM
";
    // var layout: C=0, x=1
    assert_run_i32_with(source, &options, &[(1, 42)]);
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_wider_integer_initialized_from_constant_then_value_widens(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    C : INT := -5;
END_VAR
PROGRAM main
VAR
    x : LINT := C;
END_VAR
END_PROGRAM
";
    // var layout: C=0, x=1
    assert_run_i64_with(source, &options, &[(1, -5)]);
}

#[rstest]
#[case::default(default_with_flag())]
#[case::twincat(twincat())]
fn end_to_end_when_lreal_initialized_from_real_constant_then_value_widens(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    C : REAL := 1.5;
END_VAR
PROGRAM main
VAR
    x : LREAL := C;
END_VAR
END_PROGRAM
";
    // var layout: C=0, x=1
    assert_run_f64_with(source, &options, &[(1, 1.5)]);
}

#[rstest]
#[case::default(CompilerOptions {
    allow_top_level_var_global: true,
    ..CompilerOptions::default()
})]
#[case::twincat(twincat())]
fn end_to_end_when_global_enumeration_has_bare_default_then_starts_with_it(
    #[case] options: CompilerOptions,
) {
    // A global declared against a user type reads a bare identifier as an
    // enumeration value, as a local does, and needs no flag.
    let source = "
TYPE Color : (Red, Green, Blue); END_TYPE
VAR_GLOBAL
    g : Color := Blue;
END_VAR
PROGRAM main
VAR
    x : Color := Green;
END_VAR
END_PROGRAM
";
    // var layout: g=0, x=1
    assert_run_i32_with(source, &options, &[(0, 2), (1, 1)]);
}
