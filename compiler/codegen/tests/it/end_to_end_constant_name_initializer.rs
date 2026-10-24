//! End-to-end tests of an initializer that names a constant: a `VAR`
//! initializer (`retries : UDINT := MAX_RETRIES;`), a string initializer and
//! a `TYPE` default. The smallest constant-expression initializer, enabled by
//! `--allow-constant-initializer-expressions` and by the dialects that
//! include it.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::Snapshot;

/// The default dialect with the flag. The TwinCAT dialect, which includes
/// the flag and is where the shape is common, is the other case.
fn flagged() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    }
}

#[rstest]
#[case::flag(flagged())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn end_to_end_when_initializer_names_global_constant_then_starts_at_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    MAX_RETRIES : UDINT := 3;
END_VAR

PROGRAM main
VAR
    retries : UDINT := MAX_RETRIES;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &options);

    assert_eq!(3, snapshot.read_as::<u32>("retries"));
}

#[rstest]
#[case::flag(flagged())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn end_to_end_when_initializer_names_local_constant_then_starts_at_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
PROGRAM main
VAR CONSTANT
    LIMIT : INT := 100;
    NAME : STRING := 'pump';
END_VAR
VAR
    threshold : INT := LIMIT;
    label : STRING[20] := NAME;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &options);

    assert_eq!(100, snapshot.read_as::<i32>("threshold"));
    assert_eq!(snapshot.read("label"), "pump");
}

/// A variable of a type whose default names a constant starts at the
/// constant's value.
#[rstest]
#[case::flag(flagged())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn end_to_end_when_type_default_names_constant_then_variable_starts_at_its_value(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    DEFAULT_LEVEL : UDINT := 2;
END_VAR

TYPE
    Level : UDINT := DEFAULT_LEVEL;
END_TYPE

PROGRAM main
VAR
    level : Level;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &options);

    assert_eq!(2, snapshot.read_as::<u32>("level"));
}

/// A string of either width and any length starts at the constant's text.
#[rstest]
#[case::flag(flagged())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn end_to_end_when_string_initializer_names_global_constant_then_starts_at_its_text(
    #[case] options: CompilerOptions,
) {
    let source = "
VAR_GLOBAL CONSTANT
    GREETING : STRING := 'hello';
    WIDE : WSTRING := \"wide\";
END_VAR

PROGRAM main
VAR
    s : STRING := GREETING;
    t : STRING[20] := GREETING;
    w : WSTRING[8] := WIDE;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &options);

    assert_eq!(snapshot.read("s"), "hello");
    assert_eq!(snapshot.read("t"), "hello");
    assert_eq!(snapshot.read("w"), "wide");
}

#[test]
fn end_to_end_when_string_initializer_names_sized_local_constant_then_starts_at_its_text() {
    let source = "
PROGRAM main
VAR CONSTANT
    NAME : STRING[20] := 'twenty';
END_VAR
VAR
    s : STRING[20] := NAME;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &flagged());

    assert_eq!(snapshot.read("s"), "twenty");
}

/// A function block's field starts at the constant's value too: the shape
/// the TwinCAT libraries use.
#[test]
fn end_to_end_when_function_block_field_names_global_constant_then_starts_at_its_value() {
    let source = "
VAR_GLOBAL CONSTANT
    DEFAULT_LEVEL : UDINT := 2;
END_VAR

FUNCTION_BLOCK FB_Log
VAR
    level : UDINT := DEFAULT_LEVEL;
END_VAR
VAR_OUTPUT
    current : UDINT;
END_VAR
    current := level;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    log : FB_Log;
    level : UDINT;
END_VAR
    log(current => level);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::from_dialect(Dialect::TwinCat));

    assert_eq!(2, snapshot.read_as::<u32>("level"));
}

/// The constant's value is stored at the variable's type: a widening the
/// declared types allow is applied to the value.
#[test]
fn end_to_end_when_constant_widens_to_variable_type_then_starts_at_widened_value() {
    let source = "
PROGRAM main
VAR CONSTANT
    OFFSET : INT := -5;
    SCALE : REAL := 1.5;
END_VAR
VAR
    position : DINT := OFFSET;
    factor : LREAL := SCALE;
END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &flagged());

    assert_eq!(-5, snapshot.read_as::<i32>("position"));
    assert_eq!(1.5, snapshot.read_as::<f64>("factor"));
}
