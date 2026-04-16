//! Examples demonstrating bit-access paths that were previously not
//! implemented by PR #916 (partial-access bit syntax). Each test exercises a
//! specific NotImplemented branch that needs to be filled in:
//!
//!   1. Bit write on an LWORD/LINT array element (W64 array-element path).
//!   2. Bit access on an array that is a field of a struct (the
//!      "non-trivial array base" branch).
//!   3. Bit read/write on a struct field.
//!
//! These tests fail before implementation and pass after.

mod common;
use common::{parse_and_run, try_parse_and_compile};
use ironplc_parser::options::CompilerOptions;

// --- 1. Bit write on an LWORD array element. Before the fix, this produces a
//        NotImplemented diagnostic in compile_bit_access_assignment_on_array
//        because element_vti.op_width == OpWidth::W64.

#[test]
fn end_to_end_when_write_bit_on_lword_array_element_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..1] OF LWORD;
    x : LWORD;
  END_VAR
  arr[0].0 := TRUE;
  arr[1].40 := TRUE;
  x := arr[1];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // x = arr[1]; arr[1] bit 40 = 2^40 = 1099511627776
    assert_eq!(bufs.vars[1].as_i64(), 1099511627776);
}

#[test]
fn end_to_end_when_write_bit_on_lint_array_element_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..1] OF LINT;
    x : LINT;
  END_VAR
  arr[0] := 0;
  arr[0].32 := TRUE;
  x := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // x = arr[0]; bit 32 = 2^32 = 4294967296
    assert_eq!(bufs.vars[1].as_i64(), 4294967296);
}

#[test]
fn end_to_end_when_write_bit_on_lword_array_preserves_other_bits() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..0] OF LWORD;
    x : LWORD;
  END_VAR
  arr[0] := LWORD#16#FF00_FF00_FF00_FF00;
  arr[0].0 := TRUE;
  x := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // x = arr[0]; 0xFF00FF00FF00FF00 | 0x1 = 0xFF00FF00FF00FF01
    let expected: i64 = 0xFF00_FF00_FF00_FF01_u64 as i64;
    assert_eq!(bufs.vars[1].as_i64(), expected);
}

// --- 2. Bit access on an array that is a struct field. Before the fix,
//        compile_bit_access_assignment_on_array / compile_variable_read
//        cannot resolve the array (it's not in ctx.array_vars) and produces
//        "Bit access on non-trivial array base is not yet supported".

#[test]
fn end_to_end_when_read_bit_on_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    r0 : BOOL;
    r2 : BOOL;
  END_VAR
  s.flags := BYTE#16#05;
  r0 := s.flags.0;
  r2 := s.flags.2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 0x05 = 0b00000101: bit 0 = 1, bit 2 = 1.
    assert_eq!(bufs.vars[1].as_i32(), 1);
    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_write_bit_on_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    x : BYTE;
  END_VAR
  s.flags := BYTE#16#AA;
  s.flags.0 := TRUE;
  x := s.flags;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 0xAA | 0x01 = 0xAB = 171
    assert_eq!(bufs.vars[1].as_i32(), 171);
}

#[test]
fn end_to_end_when_write_bit_on_struct_field_preserves_other_bits() {
    let source = "
TYPE MY_STRUCT : STRUCT
    a : BYTE;
    b : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    va : BYTE;
    vb : BYTE;
  END_VAR
  s.a := BYTE#16#FF;
  s.b := BYTE#16#00;
  s.a.0 := FALSE;
  s.b.7 := TRUE;
  va := s.a;
  vb := s.b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // s.a: 0xFF & 0xFE = 0xFE = 254
    // s.b: 0x00 | 0x80 = 0x80 = 128
    assert_eq!(bufs.vars[1].as_i32(), 254);
    assert_eq!(bufs.vars[2].as_i32(), 128);
}

#[test]
fn end_to_end_when_read_bit_on_word_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : WORD;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    r : BOOL;
  END_VAR
  s.flags := WORD#16#8000;
  r := s.flags.15;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_write_bit_on_dint_struct_field_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    value : DINT;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    x : DINT;
  END_VAR
  s.value := 0;
  s.value.16 := TRUE;
  x := s.value;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Set bit 16 of 0 = 65536
    assert_eq!(bufs.vars[1].as_i32(), 65536);
}

// --- 3. %Xn syntax on struct field and on LWORD array element. Gated on the
//        partial-access flag.

fn opts_with_partial_access() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_percent_x_on_struct_field_read_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    r0 : BOOL;
    r1 : BOOL;
  END_VAR
  s.flags := BYTE#16#05;
  r0 := s.flags.%X0;
  r1 := s.flags.%X1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_partial_access());
    assert_eq!(bufs.vars[1].as_i32(), 1);
    assert_eq!(bufs.vars[2].as_i32(), 0);
}

#[test]
fn end_to_end_when_percent_x_on_struct_field_write_then_correct() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
    x : BYTE;
  END_VAR
  s.flags := BYTE#16#00;
  s.flags.%X3 := TRUE;
  x := s.flags;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_partial_access());
    assert_eq!(bufs.vars[1].as_i32(), 8);
}

#[test]
fn end_to_end_when_percent_x_on_lword_array_write_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..0] OF LWORD;
    x : LWORD;
  END_VAR
  arr[0] := LWORD#0;
  arr[0].%X40 := TRUE;
  x := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_partial_access());
    // x = arr[0]
    assert_eq!(bufs.vars[1].as_i64(), 1099511627776);
}

// --- Sanity check: previously-implemented paths still work. These duplicate
// tests in end_to_end_bit_access.rs but with distinct names so a regression
// in the new implementation can be localized.

#[test]
fn end_to_end_when_write_bit_on_dint_array_then_correct() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..1] OF DINT;
    x : DINT;
  END_VAR
  arr[0] := 0;
  arr[0].16 := TRUE;
  x := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // x is the scalar we wrote arr[0] into. vars[0] is the array base, not a scalar.
    assert_eq!(bufs.vars[1].as_i32(), 65536);
}

// --- Compilation-failure sanity tests (showing what previously errored).

/// Before the fix, `arr[0].0 := TRUE;` on an LWORD array produced a
/// NotImplemented error. After the fix, it compiles successfully.
#[test]
fn end_to_end_when_write_bit_on_lword_array_then_compiles() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[0..0] OF LWORD;
  END_VAR
  arr[0].0 := TRUE;
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}

/// Before the fix, `s.flags.0 := TRUE;` produced a resolver error since the
/// bit-access codegen doesn't handle a Structured base. After the fix, it
/// compiles.
#[test]
fn end_to_end_when_write_bit_on_struct_field_then_compiles() {
    let source = "
TYPE MY_STRUCT : STRUCT
    flags : BYTE;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MY_STRUCT;
  END_VAR
  s.flags.0 := TRUE;
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}
