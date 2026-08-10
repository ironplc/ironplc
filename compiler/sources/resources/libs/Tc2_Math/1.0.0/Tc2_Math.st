(* Tc2_Math compatibility library -- extended math functions.

   This file reproduces the *interface* (names and LREAL-only signatures) of
   Beckhoff's TwinCAT Tc2_Math library for interoperability. IronPLC is an
   independent open-source project and is not affiliated with, endorsed by, or
   sponsored by Beckhoff Automation GmbH. Authored from the clean-room spec at
   specs/design/library-interfaces/tc2-math.md; see
   specs/steering/compatibility-library-authoring.md.

   LTRUNC and LMOD are bound to native VM builtins in library.toml (their
   semantics cannot be written in IEC 61131-3 source), so their bodies here
   are exactly `;` and are never compiled. MODABS and FRAC are math-dictated
   ST compositions of them. *)

(* Integer part of a floating-point number, truncated toward zero. Unlike
   TRUNC, the result stays LREAL and is not limited to an integer type's
   value range. LTRUNC(2.8) = 2.0, LTRUNC(-2.8) = -2.0. *)
FUNCTION LTRUNC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
;
END_FUNCTION

(* Floating-point modulo division returning the signed remainder (the sign
   of the dividend). Unlike MOD, operates on floating-point values and
   returns non-integer remainders. LMOD(400.56, 360.0) = 40.56,
   LMOD(-400.56, 360.0) = -40.56; LMOD(x, 0.0) is NaN. *)
FUNCTION LMOD : LREAL
VAR_INPUT
    IN1 : LREAL;
    IN2 : LREAL;
END_VAR
;
END_FUNCTION

(* Modulo division returning the unsigned modulo value within the modulo
   range [0, |IM|). MODABS(-400.56, 360.0) = 319.44. *)
FUNCTION MODABS : LREAL
VAR_INPUT
    IN : LREAL;
    IM : LREAL;
END_VAR
VAR
    remainder : LREAL;
END_VAR
    remainder := LMOD(IN, IM);
    IF remainder < 0.0 THEN
        remainder := remainder + ABS(IM);
    END_IF;
    MODABS := remainder;
END_FUNCTION

(* Fractional (decimal) component of a floating-point number; keeps the sign
   of the input. FRAC(2.8) = 0.8, FRAC(-2.8) = -0.8. *)
FUNCTION FRAC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
    FRAC := IN - LTRUNC(IN);
END_FUNCTION
