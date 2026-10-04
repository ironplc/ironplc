//! End-to-end integration tests for constant-expression `VAR` initializers
//! (e.g. `scaled : LREAL := SCALE*4.0;`), enabled by
//! `--allow-constant-initializer-expressions`.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

#[test]
fn end_to_end_when_arithmetic_initializer_then_computes_value() {
    let source = "
PROGRAM main
VAR
    d2r : LREAL := 4.25/180.0;
END_VAR
END_PROGRAM
";
    let options = CompilerOptions {
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);

    let d2r = snapshot.read_as::<f64>("d2r");
    assert!((d2r - (4.25 / 180.0)).abs() < 1e-12, "got {d2r}");
}

#[test]
fn end_to_end_when_user_constant_used_in_initializer_then_computes_value() {
    let source = "
VAR_GLOBAL CONSTANT
    SCALE : LREAL := 2.5;
END_VAR
PROGRAM main
VAR
    scaled : LREAL := SCALE*4.0;
END_VAR
END_PROGRAM
";
    let options = CompilerOptions {
        allow_top_level_var_global: true,
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);

    let scaled = snapshot.read_as::<f64>("scaled");
    assert!((scaled - (2.5 * 4.0)).abs() < 1e-12, "got {scaled}");
}
