(* Tc2_BuiltIns compatibility library -- TwinCAT built-in operator surface.

   This file reproduces the *interface* (names and signatures) of TwinCAT's
   implicit built-in conversion operators for interoperability. In TwinCAT
   these operators are always in scope and belong to no vendor library; in
   IronPLC they ship as ordinary library ST, authored from the clean-room
   spec at specs/design/library-interfaces/bool-to-string.md. IronPLC is an
   independent open-source project and is not affiliated with, endorsed by,
   or sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md. *)
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
