//! End-to-end tests for the scratch slot of a function whose result is a
//! structure holding strings in an array: an `ARRAY OF STRING` field, or an
//! array of structures with a STRING field.
//!
//! The scratch slot is one of the function's own slots, after its result, so
//! a call leaves every variable of the caller as it was. The program declares
//! variables in the slots a scratch slot numbered within the function's scope
//! alone would take.

e2e_i32!(
    end_to_end_when_function_result_holds_string_array_then_caller_variables_unchanged,
    "
TYPE S : STRUCT
  names : ARRAY[1..2] OF STRING[4];
  n : DINT;
END_STRUCT;
END_TYPE

FUNCTION F : S
  VAR_INPUT x : DINT; END_VAR
  F.names[1] := 'hi';
  F.n := x;
END_FUNCTION

PROGRAM main
  VAR a : DINT := 7; b : DINT := 9; c : DINT := 11; r : S; d : DINT; len1 : DINT; END_VAR
  r := F(x := 3);
  d := r.n;
  len1 := LEN(r.names[1]);
END_PROGRAM
",
    &[("a", 7), ("b", 9), ("c", 11), ("d", 3), ("len1", 2)],
);

e2e_i32!(
    end_to_end_when_function_result_holds_struct_array_with_string_then_caller_variables_unchanged,
    "
TYPE Item : STRUCT
  code : STRING[4];
  v : DINT;
END_STRUCT;
END_TYPE

TYPE S : STRUCT
  items : ARRAY[1..2] OF Item;
  n : DINT;
END_STRUCT;
END_TYPE

FUNCTION F : S
  VAR_INPUT x : DINT; END_VAR
  F.items[1].code := 'hi';
  F.n := x;
END_FUNCTION

PROGRAM main
  VAR a : DINT := 7; b : DINT := 9; c : DINT := 11; r : S; d : DINT; END_VAR
  r := F(x := 3);
  d := r.n;
END_PROGRAM
",
    &[("a", 7), ("b", 9), ("c", 11), ("d", 3)],
);
