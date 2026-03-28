(* P4022: BOOL variable initialized with integer literal 0 or 1.

   IEC 61131-3 strictly requires TRUE/FALSE for BOOL, but initializing
   with 0 or 1 is a universal vendor extension supported by CoDeSys,
   TwinCAT, RuSTy, and virtually every PLC runtime.

   Error: P4022 — Variable initializer type is incompatible with
          declared type (variable=debug)
   OSCAT functions affected: T_PLC_MS, T_PLC_US

   The fix should include implicit integer-to-BOOL conversion in
   --dialect rusty, treating 0 as FALSE and non-zero as TRUE in
   variable initializers.

   Compile with:
     ironplcc compile --dialect rusty -o /tmp/test.iplc benchmarks/minimal_repros/24_bool_int_initializer.st
*)

FUNCTION MY_FUNC : DWORD
VAR CONSTANT
    debug : BOOL := 0;
END_VAR
    IF debug THEN
        MY_FUNC := 1;
    ELSE
        MY_FUNC := 0;
    END_IF;
END_FUNCTION

PROGRAM main
VAR
    result : DWORD;
END_VAR
    result := MY_FUNC();
END_PROGRAM
