(* Tc2_System compatibility library -- global constants.

   This file reproduces the *interface* (the constant name PI and its value)
   of Beckhoff's TwinCAT Tc2_System library for interoperability. IronPLC is an
   independent open-source project and is not affiliated with, endorsed by, or
   sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md. *)
VAR_GLOBAL CONSTANT
    PI : LREAL := 3.1415926535897932384626433832795;
END_VAR

(* Converts a BOOL to its string representation: 'TRUE' or 'FALSE'. In
   TwinCAT this is an implicit BOOL_TO_* conversion operator; IronPLC adds
   no vendor names to its compiler tables (ADR-0042), so it ships here --
   the library real TwinCAT projects reference by default. Authored from the
   clean-room spec at specs/design/library-interfaces/bool-to-string.md. *)
FUNCTION BOOL_TO_STRING : STRING
VAR_INPUT
    IN : BOOL;
END_VAR
    IF IN THEN
        BOOL_TO_STRING := 'TRUE';
    ELSE
        BOOL_TO_STRING := 'FALSE';
    END_IF;
END_FUNCTION
