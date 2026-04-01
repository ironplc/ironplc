(* Minimal repro: integer literal passed to REAL parameter.
   Previously raised P4026 because bare literal 0 resolves as ANY_INT
   and was not inferred as REAL.

   Compile with:
     ironplcc compile benchmarks/minimal_repros/30_int_literal_to_real_param.st
*)

FUNCTION TAKES_REAL : REAL
VAR_INPUT
    x : REAL;
END_VAR
    TAKES_REAL := x;
END_FUNCTION

PROGRAM main
VAR
    r : REAL;
END_VAR
    r := TAKES_REAL(x := 0);
END_PROGRAM
