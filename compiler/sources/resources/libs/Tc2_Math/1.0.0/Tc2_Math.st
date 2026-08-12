(* Tc2_Math compatibility library -- LREAL math functions.

   This file reproduces the *interface* (names and LREAL-only signatures) of
   four functions from Beckhoff's TwinCAT 3 Tc2_Math PLC library for
   interoperability. Bodies are IronPLC-authored ST implementing the normative
   definitions in the clean-room spec at
   specs/design/library-interfaces/tc2-math.md (the sole input to this
   implementation): LTRUNC and LMOD delegate to the __TRUNC/__MOD compiler
   intrinsics, whose IEEE-754 binary64 semantics (NaN/infinity propagation,
   x mod 0.0 = NaN and never a trap, sign of the dividend, sign-of-zero
   preservation) are the spec's pinned edge-case behavior; MODABS and FRAC are
   the spec's math-dictated compositions, calling the intrinsics directly so a
   user declaration shadowing LMOD or LTRUNC cannot change their behavior.
   IronPLC is an independent open-source project and is not affiliated with,
   endorsed by, or sponsored by Beckhoff Automation GmbH. See
   specs/steering/compatibility-library-authoring.md. *)

(* Integer part of IN, rounding toward zero, staying LREAL: unlike the IEC
   TRUNC (ANY_INT result), inputs beyond any integer type's range truncate
   exactly, without clamping. *)
FUNCTION LTRUNC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
LTRUNC := __TRUNC(IN);
END_FUNCTION

(* Floating modulo returning the signed remainder (IEEE-754 fmod): the result
   has the sign of IN1 (the dividend) and |result| < |IN2|. *)
FUNCTION LMOD : LREAL
VAR_INPUT
    IN1 : LREAL;
    IN2 : LREAL;
END_VAR
LMOD := __MOD(IN1, IN2);
END_FUNCTION

(* Modulo returning the unsigned representative in [0.0, |IM|) -- the
   wrap-around used for positioning. The two-branch composition never
   produces |IM| itself: an unconditional + ABS(IM) would return |IM| for a
   zero remainder, which is outside the range. *)
FUNCTION MODABS : LREAL
VAR_INPUT
    IN : LREAL;
    IM : LREAL;
END_VAR
VAR
    remainder : LREAL;
END_VAR
remainder := __MOD(IN, IM);
IF remainder < 0.0 THEN
    MODABS := remainder + ABS(IM);
ELSE
    MODABS := remainder;
END_IF;
END_FUNCTION

(* Fractional part of IN: keeps the sign of the input, |result| < 1.0. *)
FUNCTION FRAC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
FRAC := IN - __TRUNC(IN);
END_FUNCTION
