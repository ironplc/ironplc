//! End-to-end tests for a STRING field of an array-of-struct element, such as
//! `MyBay.Devices.MeterQRScanner[i].LastCode` (#1382).
//!
//! Each element's copy of the field is one structure apart, so it is reached
//! through a strided STRING descriptor (ADR-0054), and its header is written
//! by one `STR_INIT_ARRAY` per field at initialization.
//!
//! STRING values cannot be read as a slot, so `LEN` and comparisons witness
//! them. A comparison reads the field into a plain variable first: the
//! analyzer leaves an array-of-struct element field untyped inside an
//! expression, which codegen rejects for any field type (#1791).
//!
//! Result variables are declared first: a structure holding STRING fields of
//! array elements also gets a scratch variable, which takes the slot after the
//! structure's own. Declaring results first keeps their slots at 0, 1, 2, ...

use crate::common::{parse_and_try_run, try_parse_and_compile};
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;

// Each element holds its own value, the primitive fields on either side of
// the STRING are untouched (a wrong stride would overwrite a neighbour), a
// longer value is truncated to the field's length, and the characters -- not
// only the length -- survive a copy out.
//
// len1 0, len2 1, len3 2, sum_a 3, sum_b 4, same 5.
e2e_i32!(
    end_to_end_when_element_string_fields_written_in_loop_then_elements_and_neighbours_distinct,
    "
TYPE Item : STRUCT
  a : DINT;
  code : STRING[5];
  b : DINT;
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
  items : ARRAY[1..3] OF Item;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    len1 : DINT;
    len2 : DINT;
    len3 : DINT;
    sum_a : DINT;
    sum_b : DINT;
    same : DINT;
    h : Holder;
    i : INT;
    s : STRING[5];
  END_VAR
  FOR i := 1 TO 3 DO
    h.items[i].a := i;
    h.items[i].b := 10 * i;
  END_FOR;
  h.items[1].code := 'x';
  h.items[2].code := 'yy';
  h.items[3].code := 'zzzzzzz';
  FOR i := 1 TO 3 DO
    sum_a := sum_a + h.items[i].a;
    sum_b := sum_b + h.items[i].b;
  END_FOR;
  i := 1;
  len1 := LEN(h.items[i].code);
  i := 2;
  len2 := LEN(h.items[i].code);
  i := 3;
  len3 := LEN(h.items[i].code);
  s := h.items[2].code;
  IF s = 'yy' THEN
    same := 1;
  END_IF;
END_PROGRAM
",
    &[(0, 1), (1, 2), (2, 5), (3, 6), (4, 60), (5, 1)],
);

// The array of structures sits inside a nested structure, as in the reported
// program (`MyBay.Devices.MeterQRScanner[i].LastCode`).
//
// r 0, flags 1.
e2e_i32!(
    end_to_end_when_element_string_field_in_nested_structure_written_in_loop_then_reads_back,
    "
TYPE QRScanner : STRUCT
  Trigger : BOOL;
  LastCode : STRING[50];
  ReadStatus : BOOL;
END_STRUCT;
END_TYPE

TYPE Devices : STRUCT
  Tag : STRING[20];
  MeterQRScanner : ARRAY[1..6] OF QRScanner;
END_STRUCT;
END_TYPE

TYPE Bay : STRUCT
  BayID : INT;
  Devices : Devices;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : DINT;
    flags : DINT;
    MyBay : Bay;
    i : INT;
    trig : BOOL;
    status : BOOL;
  END_VAR
  MyBay.Devices.Tag := 'tag';
  FOR i := 1 TO 6 DO
    MyBay.Devices.MeterQRScanner[i].Trigger := TRUE;
    MyBay.Devices.MeterQRScanner[i].LastCode := 'METER-CODE-001';
    MyBay.Devices.MeterQRScanner[i].ReadStatus := TRUE;
  END_FOR;
  r := LEN(MyBay.Devices.MeterQRScanner[6].LastCode);
  FOR i := 1 TO 6 DO
    trig := MyBay.Devices.MeterQRScanner[i].Trigger;
    status := MyBay.Devices.MeterQRScanner[i].ReadStatus;
    IF trig AND status THEN
      flags := flags + 1;
    END_IF;
  END_FOR;
END_PROGRAM
",
    &[(0, 14), (1, 6)],
);

// The array is a variable in its own right. The elements never written read
// as empty: their headers are initialized, so the read does not trap on a
// zero char_width.
//
// r 0, others 1.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_string_field_written_then_reads_back,
    "
TYPE Item : STRUCT
  n : DINT;
  name : STRING[8];
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : DINT;
    others : DINT;
    arr : ARRAY[1..3] OF Item;
    i : INT;
  END_VAR
  i := 2;
  arr[i].name := 'hello';
  r := LEN(arr[2].name);
  others := LEN(arr[1].name) + LEN(arr[3].name);
END_PROGRAM
",
    &[(0, 5), (1, 0)],
);

// Multi-dimensional, with two STRING fields in the element: the flat element
// index spans both dimensions, and each field has its own descriptor.
//
// r1 0, r2 1, r3 2.
e2e_i32!(
    end_to_end_when_element_string_fields_of_2d_array_written_then_reads_back,
    "
TYPE Item : STRUCT
  code : STRING[6];
  n : DINT;
  tag : STRING[2];
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
  grid : ARRAY[1..2, 0..2] OF Item;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r1 : DINT;
    r2 : DINT;
    r3 : DINT;
    h : Holder;
    i : INT;
    j : INT;
  END_VAR
  h.grid[1, 2].code := 'ab';
  i := 2;
  j := 0;
  h.grid[i, j].code := 'abcd';
  h.grid[i, j].tag := 'x';
  r1 := LEN(h.grid[1, 2].code);
  r2 := LEN(h.grid[2, 0].code);
  r3 := LEN(h.grid[2, 0].tag);
END_PROGRAM
",
    &[(0, 2), (1, 4), (2, 1)],
);

// WSTRING field: wide elements, one structure apart. The store produces the
// value at the element's wide encoding.
//
// r 0, same 1.
e2e_i32!(
    end_to_end_when_element_wstring_field_written_then_reads_back,
    "
TYPE Item : STRUCT
  n : DINT;
  label : WSTRING[10];
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
  items : ARRAY[1..3] OF Item;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : DINT;
    same : DINT;
    h : Holder;
    i : INT;
    w : WSTRING[10];
  END_VAR
  i := 3;
  h.items[i].label := \"wide\";
  r := LEN(h.items[3].label);
  w := h.items[3].label;
  IF w = \"wide\" THEN
    same := 1;
  END_IF;
END_PROGRAM
",
    &[(0, 4), (1, 1)],
);

// The descriptor counts elements, so an index one past the end traps even
// though the byte offset still lies inside the data region.
#[test]
fn end_to_end_when_element_string_field_index_past_end_then_traps() {
    let source = "
TYPE Item : STRUCT
  code : STRING[10];
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
  items : ARRAY[1..3] OF Item;
  tail : ARRAY[1..8] OF Item;
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    h : Holder;
    i : INT;
  END_VAR
  i := 4;
  h.items[i].code := 'oops';
END_PROGRAM
";
    let result = parse_and_try_run(source, &CompilerOptions::default());

    let trap = result.expect_err("expected an out-of-bounds trap").trap;
    assert!(
        matches!(
            trap,
            Trap::ArrayIndexOutOfBounds {
                index: 3,
                total_elements: 3,
                ..
            }
        ),
        "got {trap:?}"
    );
}

// An array of STRING inside the element steps by the structure and by the
// string, and a descriptor carries one stride (#1791).
#[test]
fn compile_when_element_string_array_field_indexed_then_not_implemented() {
    let source = "
TYPE Item : STRUCT
  names : ARRAY[1..2] OF STRING[8];
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    arr : ARRAY[1..3] OF Item;
    result : STRING[8];
  END_VAR
  result := arr[1].names[2];
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());

    assert_eq!(result.expect_err("expected rejection").code, "P9999");
}
