//! End-to-end tests for whole-aggregate assignment (`x := y` where both
//! sides are arrays or structures).
//!
//! IEC 61131-3 §7.3.3.1 makes assignment over a multi-element variable a
//! value copy. Before COPY_REGION an array target copied the data-region
//! offset instead, so the destination aliased the source (issue #1414).
//! Every test here reads back through *both* variables: reading only the
//! destination cannot tell a copy from an alias.

// The issue's exact reproduction. Under the defect ry was 99.
// Variable order: x=0, y=1, rx=2, ry=3.
e2e_i32!(
    end_to_end_when_array_assigned_then_source_is_not_aliased,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF DINT;
    y : ARRAY[1..2] OF DINT;
    rx : DINT;
    ry : DINT;
  END_VAR
  y[1] := 5;
  x := y;
  x[1] := 99;
  rx := x[1];
  ry := y[1];
END_PROGRAM
",
    &[(2, 99), (3, 5)],
);

// A write to the *source* after the copy must not be seen by the
// destination either -- the alias was bidirectional.
e2e_i32!(
    end_to_end_when_source_written_after_array_copy_then_destination_unchanged,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF DINT;
    y : ARRAY[1..2] OF DINT;
    rx : DINT;
    ry : DINT;
  END_VAR
  y[1] := 5;
  x := y;
  y[1] := 42;
  rx := x[1];
  ry := y[1];
END_PROGRAM
",
    &[(2, 5), (3, 42)],
);

// Every element is copied, not just the first.
e2e_i32!(
    end_to_end_when_array_copied_then_all_elements_move,
    "
PROGRAM main
  VAR
    x : ARRAY[1..4] OF DINT;
    y : ARRAY[1..4] OF DINT;
    total : DINT;
    i : DINT;
  END_VAR
  y[1] := 1;
  y[2] := 2;
  y[3] := 3;
  y[4] := 4;
  x := y;
  total := 0;
  FOR i := 1 TO 4 DO
    total := total + x[i];
  END_FOR;
END_PROGRAM
",
    &[(2, 10)],
);

// A narrower element type still occupies one slot each, so the copy length
// comes from the descriptor rather than the declared width.
e2e_i32!(
    end_to_end_when_int_array_copied_then_source_is_not_aliased,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF INT;
    y : ARRAY[1..2] OF INT;
    rx : INT;
    ry : INT;
  END_VAR
  y[2] := 7;
  x := y;
  x[2] := 21;
  rx := x[2];
  ry := y[2];
END_PROGRAM
",
    &[(2, 21), (3, 7)],
);

e2e_i64!(
    end_to_end_when_lint_array_copied_then_source_is_not_aliased,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF LINT;
    y : ARRAY[1..2] OF LINT;
    rx : LINT;
    ry : LINT;
  END_VAR
  y[1] := 5000000000;
  x := y;
  x[1] := 6000000000;
  rx := x[1];
  ry := y[1];
END_PROGRAM
",
    &[(2, 6000000000), (3, 5000000000)],
);

// Multi-dimensional arrays are a flat span, so the copy covers every cell.
e2e_i32!(
    end_to_end_when_two_dimensional_array_copied_then_source_is_not_aliased,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2, 1..3] OF DINT;
    y : ARRAY[1..2, 1..3] OF DINT;
    rx : DINT;
    ry : DINT;
  END_VAR
  y[2, 3] := 11;
  x := y;
  x[2, 3] := 22;
  rx := x[2, 3];
  ry := y[2, 3];
END_PROGRAM
",
    &[(2, 22), (3, 11)],
);

// Self-assignment is a copy_within over identical ranges: a no-op, not
// corruption.
e2e_i32!(
    end_to_end_when_array_assigned_to_itself_then_contents_preserved,
    "
PROGRAM main
  VAR
    x : ARRAY[1..3] OF DINT;
    r1 : DINT;
    r3 : DINT;
  END_VAR
  x[1] := 10;
  x[3] := 30;
  x := x;
  r1 := x[1];
  r3 := x[3];
END_PROGRAM
",
    &[(1, 10), (2, 30)],
);

// Whole-struct assignment behaved correctly before this change; these pin
// that the migration onto COPY_REGION kept it that way.
e2e_i32!(
    end_to_end_when_struct_assigned_then_source_is_not_aliased,
    "
TYPE
  Point : STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    a : Point;
    b : Point;
    ra : DINT;
    rb : DINT;
  END_VAR
  b.x := 5;
  a := b;
  a.x := 99;
  ra := a.x;
  rb := b.x;
END_PROGRAM
",
    &[(2, 99), (3, 5)],
);

// A struct field that is itself an array is inside the same region, so one
// copy moves it too.
e2e_i32!(
    end_to_end_when_struct_containing_array_copied_then_nested_elements_move,
    "
TYPE
  Bag : STRUCT
    items : ARRAY[1..3] OF DINT;
    count : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    a : Bag;
    b : Bag;
    ra : DINT;
    rb : DINT;
  END_VAR
  b.items[2] := 8;
  b.count := 3;
  a := b;
  a.items[2] := 80;
  ra := a.items[2];
  rb := b.items[2];
END_PROGRAM
",
    &[(2, 80), (3, 8)],
);

// The old push-every-slot protocol peaked at n+1 operand-stack entries. A
// struct wide enough to have blown a modest stack now costs one instruction
// and one stack slot.
e2e_i32!(
    end_to_end_when_wide_struct_copied_then_no_operand_stack_pressure,
    "
TYPE
  Wide : STRUCT
    f01 : DINT; f02 : DINT; f03 : DINT; f04 : DINT; f05 : DINT;
    f06 : DINT; f07 : DINT; f08 : DINT; f09 : DINT; f10 : DINT;
    f11 : DINT; f12 : DINT; f13 : DINT; f14 : DINT; f15 : DINT;
    f16 : DINT; f17 : DINT; f18 : DINT; f19 : DINT; f20 : DINT;
    f21 : DINT; f22 : DINT; f23 : DINT; f24 : DINT; f25 : DINT;
    f26 : DINT; f27 : DINT; f28 : DINT; f29 : DINT; f30 : DINT;
    f31 : DINT; f32 : DINT; f33 : DINT; f34 : DINT; f35 : DINT;
    f36 : DINT; f37 : DINT; f38 : DINT; f39 : DINT; f40 : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    a : Wide;
    b : Wide;
    first : DINT;
    last : DINT;
  END_VAR
  b.f01 := 1;
  b.f40 := 40;
  a := b;
  first := a.f01;
  last := a.f40;
END_PROGRAM
",
    &[(2, 1), (3, 40)],
);

// ARRAY OF STRING elements are variable-length regions with a
// [max_length][cur_length][encoding] header rather than plain slots. A byte
// copy is correct precisely because the analyzer has already required
// identical types, so each destination header is overwritten with the same
// max_length it had.
//
// The elements are staged through plain STRING variables because LEN() of an
// array element is not implemented (codegen/src/compile_string.rs).
e2e_i32!(
    end_to_end_when_string_array_copied_then_source_is_not_aliased,
    "
PROGRAM main
  VAR
    x : ARRAY[1..2] OF STRING[8];
    y : ARRAY[1..2] OF STRING[8];
    sx : STRING[8];
    sy : STRING[8];
    rx : DINT;
    ry : DINT;
  END_VAR
  y[1] := 'abc';
  x := y;
  x[1] := 'wxyz';
  sx := x[1];
  sy := y[1];
  rx := LEN(sx);
  ry := LEN(sy);
END_PROGRAM
",
    &[(4, 4), (5, 3)],
);

// An array whose elements are structures is not covered here: declaring one
// is itself unimplemented (`Unsupported array element type`,
// compile_array.rs). COPY_REGION needs no special case for it -- the
// descriptor makes it one flat span of slots like any other -- so it comes
// for free with array-of-struct support (#1383).
