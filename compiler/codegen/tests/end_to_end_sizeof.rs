//! End-to-end integration tests for the SIZEOF operator.

mod common;
use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;

fn sizeof_options() -> CompilerOptions {
    CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_sizeof_int_then_returns_2() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 2);
}

#[test]
fn end_to_end_when_sizeof_dint_then_returns_4() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 4);
}

#[test]
fn end_to_end_when_sizeof_dword_then_returns_4() {
    let source = "
PROGRAM main
  VAR
    y : DWORD;
    s : DINT;
  END_VAR
  s := SIZEOF(y);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 4);
}

#[test]
fn end_to_end_when_sizeof_bool_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    b : BOOL;
    s : DINT;
  END_VAR
  s := SIZEOF(b);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_sizeof_real_then_returns_4() {
    let source = "
PROGRAM main
  VAR
    r : REAL;
    s : DINT;
  END_VAR
  s := SIZEOF(r);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 4);
}

#[test]
fn end_to_end_when_sizeof_lreal_then_returns_8() {
    let source = "
PROGRAM main
  VAR
    r : LREAL;
    s : DINT;
  END_VAR
  s := SIZEOF(r);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    assert_eq!(bufs.vars[1].as_i32(), 8);
}

#[test]
fn end_to_end_when_sizeof_array_of_int_then_returns_total_bytes() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..10] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &sizeof_options());

    // 10 elements × 2 bytes each = 20
    assert_eq!(bufs.vars[1].as_i32(), 20);
}
