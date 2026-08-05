(* Tc2_Math compatibility library -- global constants.

   This file reproduces the *interface* (the constant name PI and its value)
   of Beckhoff's TwinCAT Tc2_Math library for interoperability. IronPLC is an
   independent open-source project and is not affiliated with, endorsed by, or
   sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md. *)
VAR_GLOBAL CONSTANT
    PI : LREAL := 3.14159265358979;
END_VAR
