//! End-to-end integration tests for declaring and calling a user-defined TIME function.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

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
    let (_c, bufs) = parse_and_run(source, &options);

    // TIME function returns T#5s = 5000 ms
    assert_eq!(bufs.vars[0].as_i64(), 5000);
}
