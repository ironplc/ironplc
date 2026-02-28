//! End-to-end integration tests for the MOD operator.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_mod_expression_then_variable_has_remainder() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 12;
  y := x MOD 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 12);
    assert_eq!(bufs.vars[1].as_i32(), 2);
}

#[test]
fn end_to_end_when_chain_of_modulos_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 100 MOD 7 MOD 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // (100 MOD 7) MOD 3 = 2 MOD 3 = 2
    assert_eq!(bufs.vars[0].as_i32(), 2);
}
