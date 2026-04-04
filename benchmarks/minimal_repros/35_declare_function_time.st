(* Declare a function named TIME (requires --allow-time-as-function-name).
   This wraps __SYSTEM_UP_TIME for OSCAT compatibility.
   Compile with:
   ironplcc compile --allow-time-as-function-name benchmarks/minimal_repros/35_declare_function_time.st
*)
FUNCTION TIME : TIME
TIME := T#0s;
END_FUNCTION

PROGRAM main
VAR
    t : TIME;
END_VAR
t := TIME();
END_PROGRAM
