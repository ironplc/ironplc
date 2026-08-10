# Library Interface Spec: `Tc2_Utilities` (LREAL_TO_FMTSTR)

This is a **clean-room interface/behavior spec** authored from the public
Beckhoff InfoSys documentation listed under *References*, per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md).
It is committed and merged as its own non-squashed git entry *before* any
implementation, and is the sole input to the implementation of the
`Tc2_Utilities` compatibility library (Tier B — clean-room interface shim).

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party. The `Tc2_Utilities` name is
used nominatively: it records whose interface this library mirrors.

## Scope

One function from Beckhoff's TwinCAT 3 `Tc2_Utilities` PLC library. This
spec pins the **interface only**: per the
[implementation plan](../../plans/2026-08-08-compatibility-library-bindings.md),
`LREAL_TO_FMTSTR` lands **declare-only** — the signature resolves during
`check`, and a *call* to it is a compile error (`P4046`) until the
native formatting implementation arrives in a follow-up. Beckhoff's exact
formal parameter names are kept (unlike the `Tc2_Math` normalization),
because mixed-case named-argument calls appear in real TwinCAT code for
this function.

## Interface

```iecst
FUNCTION LREAL_TO_FMTSTR : STRING(255)
VAR_INPUT
    in         : LREAL;
    iPrecision : INT;
    bRound     : BOOL;
END_VAR
```

## Behavior (pinned for the follow-up implementation)

Converts an `LREAL` value to a formatted string:

- `iPrecision` is the number of decimal places in the output.
- `bRound = TRUE` rounds the value to the requested precision;
  `bRound = FALSE` truncates (cuts) the remaining decimal places.
- The result is a `STRING(255)` decimal representation (no exponent
  notation for values in ordinary range).

Illustrative shape (behavioral pinning of exact digits, rounding ties, and
extreme-value handling is deferred to the follow-up plan that implements
the formatting): `LREAL_TO_FMTSTR(in := 1.23456, iPrecision := 3,
bRound := TRUE)` yields `'1.235'`, while `bRound := FALSE` yields `'1.234'`.

**Medium note (drives declare-only):** numerically-faithful float
formatting is expressible in principle but subtle; per
[ADR-0042](../../adrs/0042-library-functions-over-compiler-intrinsics.md)'s
recorded borderline case, neither ST nor a builtin is chosen yet. The
declaration ships now so the library surface exists and `check` passes on
real code; calls fail compile with `P4046` rather than ever producing
unfaithful output.

## References

Public Beckhoff InfoSys documentation (also recorded in the library's
manifest `references`):

- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_utilities/35143691.html> — `LREAL_TO_FMTSTR`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_utilities/34605835.html> — `Tc2_Utilities` overview

No vendor implementation source, exported `.library` files, or encumbered
third-party source was used as an input to this spec.
