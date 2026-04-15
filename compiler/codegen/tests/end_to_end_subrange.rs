//! End-to-end integration tests for subrange type compilation.

mod common;
use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;

#[test]
fn end_to_end_when_subrange_var_no_init_then_default_is_lower_bound() {
    let source = "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Default value for subrange is the lower bound (1)
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_subrange_var_with_init_then_uses_init_value() {
    let source = "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE := 75;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 75);
}

#[test]
fn end_to_end_when_subrange_var_assigned_then_stores_value() {
    let source = "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_subrange_var_in_expression_then_computes() {
    let source = "
TYPE
  MY_RANGE : INT (1..100);
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;
    y : DINT;
  END_VAR
  x := 10;
  y := x + 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 15);
}

#[test]
fn end_to_end_when_subrange_alias_var_then_default_is_lower_bound() {
    let source = "
TYPE
  BASE_RANGE : INT (1..100);
  ALIAS_RANGE : BASE_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : ALIAS_RANGE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Alias inherits the lower bound from the base subrange type
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_nested_subrange_alias_var_then_works() {
    let source = "
TYPE
  BASE_RANGE : INT (10..50);
  MID_RANGE : BASE_RANGE;
  TOP_RANGE : MID_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : TOP_RANGE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Nested alias resolves to the original subrange; default = 10
    assert_eq!(bufs.vars[0].as_i32(), 10);
}

#[test]
fn end_to_end_when_subrange_alias_with_init_then_uses_init() {
    let source = "
TYPE
  BASE_RANGE : INT (1..100);
  ALIAS_RANGE : BASE_RANGE;
END_TYPE

PROGRAM main
  VAR
    x : ALIAS_RANGE := 42;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_subrange_unsigned_base_then_works() {
    let source = "
TYPE
  U_RANGE : UINT (10..200);
END_TYPE

PROGRAM main
  VAR
    x : U_RANGE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Default value for unsigned subrange is the lower bound (10)
    assert_eq!(bufs.vars[0].as_i32(), 10);
}
