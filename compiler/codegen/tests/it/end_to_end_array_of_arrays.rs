//! End-to-end tests for arrays of arrays: an array whose element type is
//! itself an array type, `rows : ARRAY[1..2] OF Row` with
//! `Row : ARRAY[1..3] OF DINT`. See issue #1925.
//!
//! An array of arrays is laid out as the multi-dimensional array with the
//! same bounds, so `rows[i][j]` reads the element `[i, j]` of an
//! `ARRAY[1..2, 1..3]`.
//!
//! Assertion indices are variable-table slots, assigned in declaration order
//! starting at 0. Each test notes its mapping.

use crate::common::parse_and_run;
use ironplc_parser::options::CompilerOptions;

// rows 0, x 1.
e2e_i32!(
    end_to_end_when_array_of_arrays_element_written_then_reads_back,
    "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR
    rows : ARRAY[1..2] OF Row;
    x : DINT;
  END_VAR
  rows[2][3] := 7;
  x := rows[2][3];
END_PROGRAM
",
    &[(1, 7)],
);

// Every element written with its own value must read back unchanged: a
// wrong row stride would make two elements share storage.
//
// rows 0, i 1, j 2, r11 3, r13 4, r21 5, r23 6.
e2e_i32!(
    end_to_end_when_array_of_arrays_filled_with_variable_subscripts_then_each_element_distinct,
    "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR
    rows : ARRAY[1..2] OF Row;
    i : DINT;
    j : DINT;
    r11 : DINT;
    r13 : DINT;
    r21 : DINT;
    r23 : DINT;
  END_VAR
  FOR i := 1 TO 2 DO
    FOR j := 1 TO 3 DO
      rows[i][j] := i * 10 + j;
    END_FOR;
  END_FOR;
  r11 := rows[1][1];
  r13 := rows[1][3];
  r21 := rows[2][1];
  r23 := rows[2][3];
END_PROGRAM
",
    &[(3, 11), (4, 13), (5, 21), (6, 23)],
);

// rows 0, x 1.
e2e_i32!(
    end_to_end_when_named_array_of_arrays_element_written_then_reads_back,
    "
TYPE
  Row : ARRAY[1..3] OF DINT;
  Rows : ARRAY[0..1] OF Row;
END_TYPE
PROGRAM main
  VAR
    rows : Rows;
    x : DINT;
  END_VAR
  rows[0][1] := 5;
  rows[1][1] := 6;
  x := rows[0][1] * 10 + rows[1][1];
END_PROGRAM
",
    &[(1, 56)],
);

// rows 0, b 1, c 2, x 3.
e2e_i32!(
    end_to_end_when_array_of_arrays_element_in_expressions_then_evaluates,
    "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR
    rows : ARRAY[1..2] OF Row;
    b : BOOL;
    c : BOOL;
    x : DINT;
  END_VAR
  rows[1][2] := 4;
  rows[2][1] := -9;
  b := rows[1][2] > 3;
  c := rows[1][2] = rows[2][1];
  x := ABS(rows[2][1]) + MAX(rows[1][2], 1);
END_PROGRAM
",
    &[(1, 1), (2, 0), (3, 13)],
);

// grid 0, a 1, b 2.
e2e_i32!(
    end_to_end_when_two_dimensional_array_of_arrays_then_brackets_select_same_element,
    "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR
    grid : ARRAY[1..2, 1..2] OF Row;
    a : DINT;
    b : DINT;
  END_VAR
  grid[2, 1][3] := 8;
  grid[1, 2][1] := 3;
  a := grid[2][1][3];
  b := grid[1, 2][1];
END_PROGRAM
",
    &[(1, 8), (2, 3)],
);

// rec 0, x 1, y 2.
e2e_i32!(
    end_to_end_when_structure_field_array_of_arrays_written_then_reads_back,
    "
TYPE
  Row : ARRAY[1..3] OF DINT;
  Rec : STRUCT
    n : DINT;
    t : ARRAY[1..2] OF Row;
    m : DINT;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    rec : Rec;
    x : DINT;
    y : DINT;
  END_VAR
  rec.n := 1;
  rec.m := 2;
  rec.t[2][3] := 8;
  rec.t[1][1] := 4;
  x := rec.t[2][3] + rec.t[1][1];
  y := rec.n + rec.m;
END_PROGRAM
",
    &[(1, 12), (2, 3)],
);

// recs 0, x 1, y 2.
e2e_i32!(
    end_to_end_when_array_of_structures_field_array_of_arrays_written_then_reads_back,
    "
TYPE
  Row : ARRAY[1..3] OF DINT;
  Rec : STRUCT
    n : DINT;
    t : ARRAY[1..2] OF Row;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    recs : ARRAY[1..2] OF Rec;
    x : DINT;
    y : DINT;
  END_VAR
  recs[2].t[1][3] := 9;
  recs[1].t[2][1] := 5;
  recs[2].n := 1;
  x := recs[2].t[1][3] * 10 + recs[1].t[2][1];
  y := recs[2].n;
END_PROGRAM
",
    &[(1, 95), (2, 1)],
);

// recs 0, x 1.
e2e_i32!(
    end_to_end_when_array_of_structures_array_field_written_then_reads_back,
    "
TYPE
  Rec : STRUCT
    v : ARRAY[1..3] OF DINT;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    recs : ARRAY[1..2] OF Rec;
    x : DINT;
  END_VAR
  recs[2].v[3] := 9;
  x := recs[2].v[3];
END_PROGRAM
",
    &[(1, 9)],
);

#[test]
fn end_to_end_when_array_of_real_arrays_element_written_then_reads_back() {
    let source = "
TYPE Row : ARRAY[1..2] OF REAL; END_TYPE
PROGRAM main
  VAR
    rows : ARRAY[1..2] OF Row;
    x : REAL;
  END_VAR
  rows[2][2] := 1.5;
  x := rows[2][2] * 2.0;
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_f32(), 3.0);
}

// g 0, grid 1, h 2, x 3, y 4, z 5.
e2e_i32!(
    end_to_end_when_array_of_arrays_of_structures_field_written_then_reads_back,
    "
TYPE
  Rec : STRUCT
    n : DINT;
    v : ARRAY[1..2] OF DINT;
  END_STRUCT;
  Recs : ARRAY[1..2] OF Rec;
  Grid : ARRAY[1..3] OF Recs;
  Holder : STRUCT
    k : DINT;
    g : ARRAY[1..2] OF Recs;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    g : ARRAY[1..2] OF Recs;
    grid : Grid;
    h : Holder;
    x : DINT;
    y : DINT;
    z : DINT;
  END_VAR
  g[2][1].n := 3;
  g[1][2].n := 4;
  g[2][2].v[2] := 5;
  grid[3][2].n := 6;
  h.k := 1;
  h.g[2][1].n := 7;
  x := g[2][1].n * 10 + g[1][2].n;
  y := g[2][2].v[2] + grid[3][2].n;
  z := h.g[2][1].n + h.k;
END_PROGRAM
",
    &[(3, 34), (4, 11), (5, 8)],
);

// A structure holding a STRING array takes a scratch variable, so the
// results are declared first.
//
// n 0, m 1, same 2.
e2e_i32!(
    end_to_end_when_array_of_string_arrays_written_then_reads_back,
    "
TYPE
  Names : ARRAY[1..2] OF STRING[10];
  Rec : STRUCT
    t : ARRAY[1..2] OF Names;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    n : DINT;
    m : DINT;
    same : BOOL;
    names : ARRAY[1..2] OF Names;
    rec : Rec;
  END_VAR
  names[2][1] := 'abc';
  rec.t[2][2] := 'hello';
  n := LEN(names[2][1]);
  m := LEN(rec.t[2][2]);
  same := rec.t[2][2] = 'hello';
END_PROGRAM
",
    &[(0, 3), (1, 5), (2, 1)],
);

// Strings inside the structure elements of an array of arrays of structures
// held by a structure: each element's STRING needs its own descriptor.
//
// n 0.
e2e_i32!(
    end_to_end_when_structure_holds_array_of_arrays_of_structures_with_string_then_reads_back,
    "
TYPE
  Rec : STRUCT
    s : STRING[5];
  END_STRUCT;
  Recs : ARRAY[1..2] OF Rec;
  Holder : STRUCT
    g : ARRAY[1..2] OF Recs;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    n : DINT;
    h : Holder;
  END_VAR
  h.g[2][1].s := 'ok';
  n := LEN(h.g[2][1].s);
END_PROGRAM
",
    &[(0, 2)],
);

// rec 0, b 1.
e2e_i32!(
    end_to_end_when_bit_of_structure_field_array_of_arrays_read_then_evaluates,
    "
TYPE
  Row : ARRAY[1..2] OF BYTE;
  Rec : STRUCT
    t : ARRAY[1..2] OF Row;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    rec : Rec;
    b : BOOL;
  END_VAR
  rec.t[2][1] := 16#04;
  b := rec.t[2][1].2;
END_PROGRAM
",
    &[(1, 1)],
);
