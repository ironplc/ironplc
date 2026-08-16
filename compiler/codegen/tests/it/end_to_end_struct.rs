//! End-to-end integration tests for structure field read support.
//! Compiles ST programs with struct field access and runs them through the VM.

use crate::common::{parse_and_run, try_parse_and_compile};
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::{CompilerOptions, Dialect};

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

// --- Scalar field read/write ---

// result is var index 1 (s is var 0, result is var 1).
e2e_i32!(
    end_to_end_when_struct_field_read_then_returns_initialized_value,
    "TYPE MyStruct : STRUCT a : INT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct := (a := 10, b := 20); result : DINT; END_VAR result := s.b; END_PROGRAM",
    &[(1, 20)],
);

e2e_i32!(
    end_to_end_when_struct_field_read_first_field_then_correct_value,
    "TYPE MyStruct : STRUCT a : INT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct := (a := 10, b := 20); result : INT; END_VAR result := s.a; END_PROGRAM",
    &[(1, 10)],
);

e2e_i32!(
    end_to_end_when_struct_field_arithmetic_then_correct_result,
    "TYPE MyStruct : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct := (x := 30, y := 12); result : DINT; END_VAR result := s.x + s.y; END_PROGRAM",
    &[(1, 42)],
);

e2e_i32!(
    end_to_end_when_struct_field_read_default_init_then_returns_zero,
    "TYPE MyStruct : STRUCT a : INT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct; result : DINT; END_VAR result := s.b; END_PROGRAM",
    &[(1, 0)],
);

e2e_i32!(
    end_to_end_when_struct_field_read_bool_then_correct_value,
    "TYPE MyStruct : STRUCT flag : BOOL; count : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct := (flag := TRUE, count := 5); result_flag : DINT; result_count : DINT; END_VAR result_flag := BOOL_TO_DINT(s.flag); result_count := s.count; END_PROGRAM",
    &[(1, 1), (2, 5)],
);

// Struct with STRING field is defined but not instantiated.
e2e_i32!(
    end_to_end_when_struct_with_string_field_defined_then_program_runs,
    "TYPE MY_DATA : STRUCT NAME : STRING; VALUE : INT; END_STRUCT; END_TYPE PROGRAM main VAR x : INT; END_VAR x := 42; END_PROGRAM",
    &[(0, 42)],
);

// Regression test: global struct with STRING field previously failed with
// P9999 "Structure contains unsupported field types".
// data1 is var 0 (global), x is var 1.
e2e_i32_with!(
    end_to_end_when_global_struct_with_string_field_then_compiles_and_runs,
    CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    },
    "TYPE MY_DATA : STRUCT NAME : STRING[30]; VALUE : INT; END_STRUCT; END_TYPE VAR_GLOBAL data1 : MY_DATA; END_VAR PROGRAM main VAR x : INT; END_VAR x := 1; END_PROGRAM",
    &[(1, 1)],
);

// Struct with STRING field as local variable.
e2e_i32!(
    end_to_end_when_local_struct_with_string_field_then_compiles_and_runs,
    "TYPE MY_DATA : STRUCT NAME : STRING[30]; VALUE : INT; END_STRUCT; END_TYPE PROGRAM main VAR data1 : MY_DATA; x : INT; END_VAR x := 1; END_PROGRAM",
    &[(1, 1)],
);

// Read the INT field of a struct that also contains a STRING field.
e2e_i32!(
    end_to_end_when_struct_with_string_field_then_int_field_accessible,
    "TYPE MY_DATA : STRUCT NAME : STRING[30]; VALUE : INT; END_STRUCT; END_TYPE PROGRAM main VAR data1 : MY_DATA; result : INT; END_VAR data1.VALUE := 42; result := data1.VALUE; END_PROGRAM",
    &[(1, 42)],
);

e2e_f32!(
    end_to_end_when_struct_field_write_then_value_stored,
    "TYPE MY_POINT : STRUCT X : REAL; Y : REAL; END_STRUCT; END_TYPE PROGRAM main VAR pt : MY_POINT; result : REAL; END_VAR pt.X := 1.0; result := pt.X; END_PROGRAM",
    &[(1, 1.0)],
);

e2e_f32!(
    end_to_end_when_struct_field_write_both_fields_then_correct_values,
    "TYPE MY_POINT : STRUCT X : REAL; Y : REAL; END_STRUCT; END_TYPE PROGRAM main VAR pt : MY_POINT; rx : REAL; ry : REAL; END_VAR pt.X := 1.0; pt.Y := 2.0; rx := pt.X; ry := pt.Y; END_PROGRAM",
    &[(1, 1.0), (2, 2.0)],
);

e2e_i32!(
    end_to_end_when_struct_field_write_int_then_correct_value,
    "TYPE MyStruct : STRUCT a : INT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct; result : DINT; END_VAR s.a := 42; s.b := 100; result := s.a + s.b; END_PROGRAM",
    &[(1, 142)],
);

// --- Array fields ---

e2e_i32!(
    end_to_end_when_struct_array_field_read_constant_index_then_correct_element,
    "TYPE MyStruct : STRUCT values : ARRAY[0..2] OF DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct; result : DINT; END_VAR s.values[0] := 10; s.values[1] := 20; s.values[2] := 30; result := s.values[1]; END_PROGRAM",
    &[(1, 20)],
);

e2e_i32!(
    end_to_end_when_struct_array_field_write_then_stores_value,
    "TYPE MyStruct : STRUCT data : ARRAY[1..3] OF DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct; result : DINT; END_VAR s.data[1] := 100; s.data[2] := 200; s.data[3] := 300; result := s.data[1] + s.data[2] + s.data[3]; END_PROGRAM",
    &[(1, 600)],
);

e2e_f32!(
    end_to_end_when_struct_array_field_variable_index_then_correct,
    "TYPE MyStruct : STRUCT items : ARRAY[0..4] OF REAL; END_STRUCT; END_TYPE PROGRAM main VAR s : MyStruct; i : INT; result : REAL; END_VAR s.items[0] := 1.0; s.items[1] := 2.0; s.items[2] := 3.0; s.items[3] := 4.0; s.items[4] := 5.0; i := 3; result := s.items[i]; END_PROGRAM",
    &[(2, 4.0)],
);

e2e_i32!(
    end_to_end_when_struct_with_scalar_and_array_fields_then_both_correct,
    "TYPE Mixed : STRUCT count : DINT; values : ARRAY[0..2] OF DINT; END_STRUCT; END_TYPE PROGRAM main VAR m : Mixed; result : DINT; END_VAR m.count := 3; m.values[0] := 10; m.values[1] := 20; m.values[2] := 30; result := m.count + m.values[0] + m.values[1] + m.values[2]; END_PROGRAM",
    &[(1, 63)],
);

// --- STRING-array fields (use `read_string` helper; stay inline) ---

#[test]
fn end_to_end_when_struct_string_array_field_write_and_read_then_correct() {
    let source = "
TYPE MyStruct :
  STRUCT
    names : ARRAY[1..3] OF STRING[10];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
    result : STRING[10];
  END_VAR
    s.names[1] := 'hello';
    s.names[2] := 'world';
    result := s.names[2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // s is var 0 (struct data_offset), result is var 1 (STRING in data region).
    // The struct occupies slots for the string array: 3 * ceil((4+10)/8) = 3*2 = 6 slots = 48 bytes.
    // result is a STRING[10] starting at data_region offset 48.
    let struct_base = bufs.vars[0].as_i32() as usize;
    let stride = STRING_HEADER_BYTES + 10;

    // Verify the writes landed in the struct's data region.
    assert_eq!(read_string(&bufs.data_region, struct_base), "hello");
    assert_eq!(
        read_string(&bufs.data_region, struct_base + stride),
        "world"
    );

    // Verify the read-back via result.
    let result_offset = struct_base + 6 * 8; // after struct's 6 slots
    assert_eq!(read_string(&bufs.data_region, result_offset), "world");
}

#[test]
fn end_to_end_when_struct_multidim_string_array_field_read_then_correct() {
    let source = "
TYPE MyLang :
  STRUCT
    names : ARRAY[1..2, 1..3] OF STRING[10];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    lang : MyLang;
    r1 : STRING[10];
    r2 : STRING[10];
  END_VAR
    lang.names[1, 1] := 'Mon';
    lang.names[1, 2] := 'Tue';
    lang.names[1, 3] := 'Wed';
    lang.names[2, 1] := 'Mo';
    lang.names[2, 2] := 'Di';
    lang.names[2, 3] := 'Mi';
    r1 := lang.names[1, 2];
    r2 := lang.names[2, 3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // lang struct: 2*3=6 STRING[10] elements, each 2 slots = 12 slots = 96 bytes.
    // r1 starts at offset 96, r2 at 96 + (4+10) = 96 + 14 = not quite...
    // r1 and r2 are STRING[10] variables, each takes STRING_HEADER_BYTES + 10 bytes.
    let struct_base = bufs.vars[0].as_i32() as usize;
    let struct_slots = 12; // 6 elements * 2 slots each
    let r1_offset = struct_base + struct_slots * 8;
    let r2_offset = r1_offset + STRING_HEADER_BYTES + 10;

    assert_eq!(read_string(&bufs.data_region, r1_offset), "Tue");
    assert_eq!(read_string(&bufs.data_region, r2_offset), "Mi");
}

#[test]
fn end_to_end_when_global_struct_string_array_field_read_then_correct() {
    let source = "
TYPE MY_LANG :
  STRUCT
    NAMES : ARRAY[1..2, 1..3] OF STRING[10];
  END_STRUCT;
END_TYPE

VAR_GLOBAL
    lang : MY_LANG;
END_VAR

PROGRAM main
  VAR
    r : STRING[10];
  END_VAR
    lang.NAMES[1, 1] := 'Mon';
    lang.NAMES[2, 2] := 'Di';
    r := lang.NAMES[2, 2];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        },
    );

    // Global struct 'lang' is var 0 (globals come first), scratch is var 1.
    // Program var 'r' is var 2.
    // Struct: 6 STRING[10] elements * 2 slots = 12 slots = 96 bytes.
    // r starts at offset 96.
    let struct_base = bufs.vars[0].as_i32() as usize;
    let struct_slots = 12;
    let r_offset = struct_base + struct_slots * 8;

    assert_eq!(read_string(&bufs.data_region, r_offset), "Di");
}

// --- Functions that return structs ---

e2e_f32!(
    end_to_end_when_function_returns_struct_with_field_assignment_then_fields_correct,
    "TYPE POINT : STRUCT X : REAL; Y : REAL; END_STRUCT; END_TYPE FUNCTION MAKE_POINT : POINT VAR_INPUT px : REAL; py : REAL; END_VAR MAKE_POINT.X := px; MAKE_POINT.Y := py; END_FUNCTION PROGRAM main VAR p : POINT; rx : REAL; ry : REAL; END_VAR p := MAKE_POINT(px := 1.5, py := 2.5); rx := p.X; ry := p.Y; END_PROGRAM",
    &[(1, 1.5), (2, 2.5)],
);

#[test]
fn end_to_end_when_struct_string_field_write_then_value_stored() {
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING[10];
    VALUE : INT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    d : MY_DATA;
  END_VAR
    d.NAME := 'hello';
    d.VALUE := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let struct_base = bufs.vars[0].as_i32() as usize;
    assert_eq!(read_string(&bufs.data_region, struct_base), "hello");
    // INT field is at slot 2 (STRING[10] = ceil((4+10)/8) = 2 slots)
    assert_eq!(bufs.vars[0].as_i32() as usize, struct_base);
}

#[test]
fn end_to_end_when_struct_string_field_read_then_correct_value() {
    let source = "
TYPE MY_DATA :
  STRUCT
    NAME : STRING[10];
    VALUE : INT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    d : MY_DATA;
    result : STRING[10];
  END_VAR
    d.NAME := 'world';
    result := d.NAME;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let struct_base = bufs.vars[0].as_i32() as usize;
    assert_eq!(read_string(&bufs.data_region, struct_base), "world");
    // result is the second string variable; its data follows the struct data
    // struct occupies 3 slots (2 for STRING[10] + 1 for INT) = 24 bytes
    let result_offset = struct_base + 3 * 8;
    assert_eq!(read_string(&bufs.data_region, result_offset), "world");
}

#[test]
fn end_to_end_when_function_return_struct_with_string_field_then_correct() {
    let source = "
TYPE MY_DATA :
  STRUCT
    TYP : BYTE;
    NAME : STRING[10];
    VALUES : ARRAY[1..3] OF DINT;
  END_STRUCT;
END_TYPE

FUNCTION MAKE_DATA : MY_DATA
VAR_INPUT
    t : BYTE;
    n : STRING[10];
END_VAR
    MAKE_DATA.TYP := t;
    MAKE_DATA.NAME := n;
    MAKE_DATA.VALUES[1] := 10;
    MAKE_DATA.VALUES[2] := 20;
    MAKE_DATA.VALUES[3] := 30;
END_FUNCTION

PROGRAM main
  VAR
    d : MY_DATA;
    result_name : STRING[10];
    result_sum : DINT;
  END_VAR
    d := MAKE_DATA(t := BYTE#5, n := 'test');
    result_name := d.NAME;
    result_sum := d.VALUES[1] + d.VALUES[2] + d.VALUES[3];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // d is var 0 (struct), result_name is var 1 (STRING), result_sum is var 2
    let struct_base = bufs.vars[0].as_i32() as usize;
    // TYP is at slot 0, NAME starts at slot 1 (2 slots for STRING[10]),
    // VALUES at slot 3 (3 DINT slots)
    // Total struct slots: 1 + 2 + 3 = 6

    // Verify NAME field in the struct data region (slot 1 = byte offset 8)
    assert_eq!(read_string(&bufs.data_region, struct_base + 8), "test");

    // Verify result_name (STRING var after struct data)
    let result_name_offset = struct_base + 6 * 8;
    assert_eq!(read_string(&bufs.data_region, result_name_offset), "test");

    // Verify sum of array values
    assert_eq!(bufs.vars[2].as_i32(), 60);
}

// Two calls to a struct-returning function should produce independent copies.
// p1 is var 0, p2 is var 1, r1x is var 2, r1y is var 3, r2x is var 4, r2y is var 5.
e2e_f32!(
    end_to_end_when_two_calls_to_struct_returning_function_then_independent_copies,
    "TYPE POINT : STRUCT X : REAL; Y : REAL; END_STRUCT; END_TYPE FUNCTION MAKE_POINT : POINT VAR_INPUT px : REAL; py : REAL; END_VAR MAKE_POINT.X := px; MAKE_POINT.Y := py; END_FUNCTION PROGRAM main VAR p1 : POINT; p2 : POINT; r1x : REAL; r1y : REAL; r2x : REAL; r2y : REAL; END_VAR p1 := MAKE_POINT(px := 1.0, py := 2.0); p2 := MAKE_POINT(px := 3.0, py := 4.0); r1x := p1.X; r1y := p1.Y; r2x := p2.X; r2y := p2.Y; END_PROGRAM",
    &[(2, 1.0), (3, 2.0), (4, 3.0), (5, 4.0)],
);

// Regression for `compile_expr.rs#L32` TODO on `struct.field[i, j] = x`.
// The analyzer previously failed to set `resolved_type` for array
// subscripts rooted in a struct field, so codegen's condition path hit
// the "missing resolved_type" branch when a 2-D STRING array field was
// compared inside an IF.
// Global DATA is var 0, scratch is var 1, r_match is var 2, r_mismatch is var 3.
e2e_i32_with!(
    end_to_end_when_struct_2d_string_array_field_compared_then_matches,
    CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    },
    "TYPE MY_DATA : STRUCT DIRS : ARRAY[0..2, 0..15] OF STRING[3]; END_STRUCT; END_TYPE VAR_GLOBAL DATA : MY_DATA; END_VAR FUNCTION FOO : INT VAR_INPUT DIR : STRING[3]; END_VAR VAR i : INT; j : INT; END_VAR FOO := 0; IF DATA.DIRS[i, j] = DIR THEN FOO := 1; END_IF; END_FUNCTION PROGRAM main VAR r_match : INT; r_mismatch : INT; END_VAR DATA.DIRS[0, 0] := 'N'; r_match := FOO(DIR := 'N'); r_mismatch := FOO(DIR := 'S'); END_PROGRAM",
    &[(2, 1), (3, 0)],
);

// --- 64-bit integer and LTIME struct fields ---
// Exercise `resolve_field_op_type` / `var_type_info_for_field` on 64-bit
// signed types (Int B64 and Time B64) — the Signed W64 branch only fires
// when these types are declared inside a STRUCT.

e2e_i64!(
    end_to_end_when_struct_field_lint_then_reads_and_writes,
    "
TYPE MyStruct : STRUCT v : LINT; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : LINT;
  END_VAR
    s.v := LINT#9000000000;
    result := s.v;
END_PROGRAM
",
    &[(1, 9_000_000_000)],
);

#[test]
fn end_to_end_when_struct_field_ltime_then_reads_and_writes() {
    let source = "
TYPE MyStruct : STRUCT t : LTIME; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : LTIME;
  END_VAR
    s.t := LTIME#5s;
    result := s.t;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    assert_eq!(bufs.vars[1].as_i64(), 5_000);
}

// --- DATE, TOD, DT struct fields ---
// Exercise the Date / TimeOfDay / DateAndTime patterns in
// `resolve_field_op_type` and `var_type_info_for_field`.

#[test]
fn end_to_end_when_struct_field_date_then_reads_and_writes() {
    let source = "
TYPE MyStruct : STRUCT d : DATE; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : DATE;
  END_VAR
    s.d := D#2024-01-01;
    result := s.d;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 2024-01-01 is 19723 days after 1970-01-01 = 1_704_067_200 seconds
    assert_eq!(bufs.vars[1].as_i32() as u32, 1_704_067_200);
}

#[test]
fn end_to_end_when_struct_field_tod_then_reads_and_writes() {
    let source = "
TYPE MyStruct : STRUCT t : TIME_OF_DAY; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : TIME_OF_DAY;
  END_VAR
    s.t := TOD#12:30:00;
    result := s.t;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 12h * 3600000 + 30m * 60000 = 45_000_000 ms
    assert_eq!(bufs.vars[1].as_i32() as u32, 45_000_000);
}

#[test]
fn end_to_end_when_struct_field_dt_then_reads_and_writes() {
    let source = "
TYPE MyStruct : STRUCT v : DATE_AND_TIME; END_STRUCT; END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : DATE_AND_TIME;
  END_VAR
    s.v := DT#2024-01-01-12:30:00;
    result := s.v;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // 1_704_067_200 + 12*3600 + 30*60 = 1_704_112_200
    assert_eq!(bufs.vars[1].as_i32() as u32, 1_704_112_200);
}

// --- Subrange struct field default value ---
// The default for a subrange is its lower bound (IEC 61131-3 §2.4.3.1).
// Exercises `emit_default_for_field` — W32 branch for INT-based subranges
// and W64 branch for LINT-based subranges.

e2e_i32!(
    end_to_end_when_struct_field_subrange_int_default_then_lower_bound,
    "
TYPE
  MY_RANGE : INT (5..50);
  MyStruct : STRUCT v : MY_RANGE; END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : INT;
  END_VAR
    result := s.v;
END_PROGRAM
",
    &[(1, 5)],
);

e2e_i64!(
    end_to_end_when_struct_field_subrange_lint_default_then_lower_bound,
    "
TYPE
  MY_RANGE : LINT (10..10000);
  MyStruct : STRUCT v : MY_RANGE; END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
    result : LINT;
  END_VAR
    result := s.v;
END_PROGRAM
",
    &[(1, 10)],
);

// --- Nested struct initialization ---
// Exercises the recursive `initialize_struct_fields` call path for nested
// struct fields, with and without explicit inner initializers.

e2e_i32!(
    end_to_end_when_nested_struct_with_explicit_inner_init_then_values_stored,
    "
TYPE
  Inner : STRUCT x : DINT; y : DINT; END_STRUCT;
  Outer : STRUCT inner : Inner; z : DINT; END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    o : Outer := (inner := (x := 1, y := 2), z := 3);
    rx : DINT;
    ry : DINT;
    rz : DINT;
  END_VAR
    rx := o.inner.x;
    ry := o.inner.y;
    rz := o.z;
END_PROGRAM
",
    &[(1, 1), (2, 2), (3, 3)],
);

// Covers the recursive `initialize_struct_fields` path when the inner
// struct has no explicit initializer — inner leaf fields still get
// zero-initialized via the default-value branch.
e2e_i32!(
    end_to_end_when_nested_struct_without_explicit_init_then_zero_defaults,
    "
TYPE
  Inner : STRUCT x : DINT; y : DINT; END_STRUCT;
  Outer : STRUCT inner : Inner; z : DINT; END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    o : Outer;
    rx : DINT;
    ry : DINT;
    rz : DINT;
  END_VAR
    rx := o.inner.x;
    ry := o.inner.y;
    rz := o.z;
END_PROGRAM
",
    &[(1, 0), (2, 0), (3, 0)],
);

#[test]
fn end_to_end_when_struct_init_value_is_expression_then_returns_not_implemented() {
    // A struct/FB-instance field initializer whose value is a general
    // (possibly non-constant) expression -- e.g. a pointer dereference
    // plus member access -- fully parses and analyzes, but codegen does
    // not yet implement evaluating it at instance construction time.
    // `ironplcc check` already fully supports this; only codegen refuses.
    // See specs/plans/2026-07-26-twincat-struct-init-expression-value.md.
    let source = "
FUNCTION_BLOCK FB_Device
VAR_INPUT
    Delta : INT;
END_VAR
END_FUNCTION_BLOCK

TYPE MyStruct :
STRUCT
    x : INT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    pDevice : REF_TO FB_Device;
    s : MyStruct := (x := pDevice^.Delta);
END_VAR
END_PROGRAM
";
    // `allow_struct_initializer_expressions` lets analysis accept the
    // expression-valued initializer so the test reaches codegen; codegen is
    // what returns `not_implemented` (P9999). Without the flag, analysis
    // would reject it earlier with P4043.
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_struct_initializer_expressions: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(source, &options);

    assert!(
        result.is_err(),
        "expected compilation to fail for an expression-valued struct init"
    );
    assert_eq!(result.unwrap_err().code, "P9999");
}

// --- Field access on array-of-struct elements ---
// `s.arr[i].field`. See specs/plans/2026-08-16-array-of-struct-field-codegen.md.

// h is var 0, result is var 1.
e2e_i32!(
    end_to_end_when_array_of_struct_field_written_with_literal_index_then_reads_back,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; result : DINT; END_VAR h.items[2].b := 42; result := h.items[2].b; END_PROGRAM",
    &[(1, 42)],
);

// Writing one element must not disturb its neighbours -- this is what a wrong
// element stride would break.
e2e_i32!(
    end_to_end_when_array_of_struct_elements_written_then_each_element_distinct,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; r1 : DINT; r2 : DINT; r3 : DINT; END_VAR h.items[1].a := 11; h.items[2].a := 22; h.items[3].a := 33; r1 := h.items[1].a; r2 := h.items[2].a; r3 := h.items[3].a; END_PROGRAM",
    &[(1, 11), (2, 22), (3, 33)],
);

// Distinct fields within one element must not alias -- this is what a wrong
// leaf offset would break.
e2e_i32!(
    end_to_end_when_array_of_struct_sibling_fields_written_then_do_not_alias,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; ra : DINT; rb : DINT; END_VAR h.items[2].a := 7; h.items[2].b := 9; ra := h.items[2].a; rb := h.items[2].b; END_PROGRAM",
    &[(1, 7), (2, 9)],
);

// Variable subscript exercises the runtime flat-index path rather than the
// compile-time constant-folded one.
e2e_i32!(
    end_to_end_when_array_of_struct_indexed_by_variable_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; i : INT; result : DINT; END_VAR i := 3; h.items[i].b := 55; result := h.items[i].b; END_PROGRAM",
    &[(2, 55)],
);

// A FOR loop over the array, mirroring the shape reported in issue #1376.
e2e_i32!(
    end_to_end_when_array_of_struct_written_in_for_loop_then_all_elements_set,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; i : INT; r : DINT; END_VAR FOR i := 1 TO 3 DO h.items[i].a := 5; END_FOR; r := h.items[3].a; END_PROGRAM",
    &[(2, 5)],
);

// Several element reads combined in one expression.
e2e_i32!(
    end_to_end_when_array_of_struct_elements_summed_then_correct_total,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; total : DINT; END_VAR h.items[1].a := 1; h.items[2].a := 2; h.items[3].a := 3; total := h.items[1].a + h.items[2].a + h.items[3].a; END_PROGRAM",
    &[(1, 6)],
);

// The array field is not the first member, so the field offset must be added
// on top of the element stride.
e2e_i32!(
    end_to_end_when_array_of_struct_preceded_by_scalar_field_then_offset_correct,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT lead : DINT; items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; rl : DINT; ra : DINT; END_VAR h.lead := 99; h.items[1].a := 4; rl := h.lead; ra := h.items[1].a; END_PROGRAM",
    &[(1, 99), (2, 4)],
);

// Nested struct chain ahead of the subscript, as in `MyBay.Devices.Scanner[i].F`.
e2e_i32!(
    end_to_end_when_array_of_struct_reached_through_nested_struct_then_correct_value,
    "TYPE Item : STRUCT a : DINT; END_STRUCT; END_TYPE TYPE Inner : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE TYPE Outer : STRUCT inner : Inner; END_STRUCT; END_TYPE PROGRAM main VAR o : Outer; result : DINT; END_VAR o.inner.items[3].a := 77; result := o.inner.items[3].a; END_PROGRAM",
    &[(1, 77)],
);

// A BOOL leaf exercises the narrow-width truncation path.
e2e_i32!(
    end_to_end_when_array_of_struct_bool_field_written_then_reads_back,
    "TYPE Item : STRUCT flag : BOOL; n : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; result : DINT; END_VAR h.items[2].flag := TRUE; result := BOOL_TO_DINT(h.items[2].flag); END_PROGRAM",
    &[(1, 1)],
);

// Two-dimensional array-of-struct: both strides must be scaled.
e2e_i32!(
    end_to_end_when_two_dimensional_array_of_struct_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Holder : STRUCT items : ARRAY[1..2, 1..3] OF Item; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; r1 : DINT; r2 : DINT; END_VAR h.items[1,1].a := 1; h.items[2,3].a := 6; r1 := h.items[1,1].a; r2 := h.items[2,3].a; END_PROGRAM",
    &[(1, 1), (2, 6)],
);

// Regression: a struct array field at slot offset 0 emits `load_const 0; add`,
// which the peephole optimizer removes. Inside a FOR loop the CMP_BR branch
// offset then had to be rewritten -- before that was handled the branch landed
// mid-instruction and the VM trapped with InvalidConstantIndex.
e2e_i32!(
    end_to_end_when_struct_array_field_written_in_for_loop_then_no_fault,
    "TYPE Holder : STRUCT vals : ARRAY[1..3] OF DINT; END_STRUCT; END_TYPE PROGRAM main VAR h : Holder; i : INT; r : DINT; END_VAR FOR i := 1 TO 3 DO h.vals[i] := 5; END_FOR; r := h.vals[3]; END_PROGRAM",
    &[(2, 5)],
);
