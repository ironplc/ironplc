(* Tc2_BuiltIns compatibility library -- TwinCAT's implicit built-in surface.

   This file reproduces the *interface* (names and signatures) of conversion
   operators that TwinCAT provides to every PLC project without a library
   reference, so that the same source compiles in both environments. Bodies
   are IronPLC-authored ST dictated by the documented behavior. IronPLC is an
   independent open-source project and is not affiliated with, endorsed by, or
   sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md and the clean-room spec
   at specs/design/library-interfaces/tc2-builtins.md. *)
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
