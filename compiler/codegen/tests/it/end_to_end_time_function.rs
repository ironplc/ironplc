//! End-to-end integration tests for declaring and calling a user-defined TIME function.

use ironplc_parser::options::CompilerOptions;

use crate::common::{Duration, Snapshot};

#[test]
fn end_to_end_when_time_function_declared_then_callable() {
    let source = "
FUNCTION TIME : TIME
TIME := T#5s;
END_FUNCTION

PROGRAM main
  VAR
    t : TIME;
  END_VAR
  t := TIME();
END_PROGRAM
";
    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);

    // TIME function returns T#5s
    assert_eq!(snapshot.read("t"), Duration::seconds(5));
}
