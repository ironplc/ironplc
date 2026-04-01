(* P4017: Time/date type conversion functions not declared.

   IEC 61131-3 defines type conversion functions between elementary
   types using the naming convention TYPE1_TO_TYPE2. IronPLC supports
   numeric conversions (INT_TO_REAL, etc.) but is missing conversions
   involving TIME, DATE, TOD, and DT types.

   These are used extensively in OSCAT for date/time arithmetic.
   In RuSTy, some are built-in and others are declared in stubs.st
   as EXTERNAL functions provided by the runtime.

   Error: P4017 - Function is not declared
   OSCAT functions affected: most date/time functions

   Compile with:
     ironplcc compile benchmarks/minimal_repros/31_time_date_conversions.st
*)

PROGRAM main
VAR
    t : TIME := T#5s;
    d : DATE := D#2024-06-15;
    tod1 : TOD := TOD#12:30:00;
    dt1 : DT := DT#2024-06-15-12:30:00;
    dw : DWORD;
    ud : UDINT;
    di : DINT;
    r : REAL;
    i : INT;
    tm : TIME;
END_VAR
    dw := TIME_TO_DWORD(t);
    tm := DWORD_TO_TIME(dw);
    r := TIME_TO_REAL(t);
    di := TIME_TO_DINT(t);
    i := TIME_TO_INT(t);
    dw := DATE_TO_DWORD(d);
    ud := DATE_TO_UDINT(d);
    dw := TOD_TO_DWORD(tod1);
    dw := DT_TO_DWORD(dt1);
END_PROGRAM
