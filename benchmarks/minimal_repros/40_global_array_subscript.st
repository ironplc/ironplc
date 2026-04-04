(* Minimal repro: array subscript on global struct member.
   Previously failed with P9999 "Not implemented" in codegen.

   Compile with:
     ironplcc compile benchmarks/minimal_repros/40_global_array_subscript.st
*)

TYPE MY_MATH :
STRUCT
    FACTS : ARRAY[0..10] OF REAL;
END_STRUCT
END_TYPE

VAR_GLOBAL CONSTANT
    math : MY_MATH;
END_VAR

FUNCTION LOOKUP : REAL
VAR_INPUT
    x : INT;
END_VAR
    LOOKUP := math.FACTS[x];
END_FUNCTION

PROGRAM main
VAR
    r : REAL;
END_VAR
    r := LOOKUP(x := 5);
END_PROGRAM
