# Library Interface Spec: `Tc2_Math` (LTRUNC, LMOD, MODABS, FRAC)

This is a **clean-room interface/behavior spec** authored from the public
Beckhoff InfoSys documentation listed under *References*, per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md).
It is committed and merged as its own non-squashed git entry *before* any
implementation, and is the sole input to the implementation of the
`Tc2_Math` compatibility library (Tier B — clean-room interface shim; the
`MODABS`/`FRAC` bodies are Tier A math-dictated compositions).

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party. The `Tc2_Math` name is used
nominatively: it records whose interface this library mirrors.

## Scope

Four functions from Beckhoff's TwinCAT 3 `Tc2_Math` PLC library. Beckhoff's
signatures are `LREAL`-only (not generic over `ANY_REAL`); this library
matches that exactly. Per the project convention established in review of
the original contributions, formal parameter names are normalized to the
IronPLC stdlib `IN`/`IN1`/`IN2` convention — real call sites use positional
arguments for these functions, and IronPLC's stdlib does not preserve vendor
formal names for any other function either. (For the record, Beckhoff's
documented formal names are `lr_in` for `LTRUNC`/`FRAC` and
`lr_Value`/`lr_Arg` for `LMOD`.)

## Interface

```iecst
FUNCTION LTRUNC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR

FUNCTION LMOD : LREAL
VAR_INPUT
    IN1 : LREAL;  (* value *)
    IN2 : LREAL;  (* modulo range *)
END_VAR

FUNCTION MODABS : LREAL
VAR_INPUT
    IN : LREAL;  (* value *)
    IM : LREAL;  (* modulo range *)
END_VAR

FUNCTION FRAC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
```

## Behavior

### `LTRUNC` — LREAL-preserving truncation

Determines the integer part of a floating-point number. Unlike the IEC
61131-3 `TRUNC` (whose result is `ANY_INT` and therefore clamped to an
integer type's value range), the result is of type `LREAL` and is not
limited to the integer range. For positive inputs the result is less than
or equal to the input; for negative inputs it is greater than or equal
(truncation toward zero, IEEE-754 `trunc`).

Pinned examples:

| Call | Result |
|---|---|
| `LTRUNC(2.8)` | `2.0` |
| `LTRUNC(-2.8)` | `-2.0` |
| `LTRUNC(3.7)` | `3.0` |
| `LTRUNC(-3.7)` | `-3.0` |

**Inexpressibility note (drives the implementation medium):** ST cannot
express this — the only truncation primitive, `TRUNC`, leaves the `LREAL`
domain and clamps to an integer range. `LTRUNC` is therefore bound to a
native VM builtin (`trunc_lreal`), reached only via the library's manifest
binding per [ADR-0042](../../adrs/0042-library-functions-over-compiler-intrinsics.md).

### `LMOD` — floating-point modulo, signed remainder

Carries out a modulo division and returns the **signed** remainder. Unlike
the integer-only IEC `MOD`, `LMOD` operates on floating-point values and
returns non-integer remainders.

Pinned semantics: IEEE-754 floating remainder with the **sign of the
dividend** (C `fmod`; Rust `%` on `f64`):

- `LMOD(IN1, IN2) = IN1 - LTRUNC(IN1 / IN2) * IN2` for finite nonzero `IN2`.
- The result has the sign of `IN1` (or is `0.0`).
- `LMOD(x, 0.0)` is **NaN** — not a runtime trap.

Pinned examples:

| Call | Result |
|---|---|
| `LMOD(400.56, 360.0)` | `≈ 40.56` |
| `LMOD(-400.56, 360.0)` | `≈ -40.56` |
| `LMOD(x, 0.0)` | `NaN` |

**Inexpressibility note:** no floating modulo exists in ST (`MOD` is
integer). `LMOD` is bound to a native VM builtin (`fmod_lreal`) via the
manifest binding.

### `MODABS` — modulo, unsigned result within the modulo range

Performs a modulo division and determines the **unsigned** modulo value
within the modulo range `[0, |IM|)` — the wrap-around behavior used for
positioning (e.g. normalizing an angle to `[0, 360)`).

Math-dictated definition in terms of `LMOD`:

```iecst
MODABS := LMOD(IN, IM);
IF MODABS < 0.0 THEN
    MODABS := MODABS + ABS(IM);
END_IF;
```

Pinned examples:

| Call | Result |
|---|---|
| `MODABS(400.56, 360.0)` | `≈ 40.56` |
| `MODABS(-400.56, 360.0)` | `≈ 319.44` |
| `MODABS(x, 0.0)` | `NaN` (propagated from `LMOD`) |

**Medium:** ST body in the library (expressible; no builtin, no func_id).

### `FRAC` — fractional part

Determines the decimal (fractional) component of a floating-point number.
The result keeps the **sign of the input**.

Math-dictated definition in terms of `LTRUNC`:

```iecst
FRAC := IN - LTRUNC(IN);
```

Pinned examples:

| Call | Result |
|---|---|
| `FRAC(2.8)` | `≈ 0.8` |
| `FRAC(-2.8)` | `≈ -0.8` |
| `FRAC(3.7)` | `≈ 0.7` |
| `FRAC(-3.7)` | `≈ -0.7` |

**Medium:** ST body in the library (expressible; no builtin, no func_id).

## Non-goals

- `REAL` (F32) variants or `ANY_REAL` generic signatures — Beckhoff's
  `Tc2_Math` signatures are `LREAL`-only; REAL arguments are served by the
  separately-planned call-site REAL→LREAL widening.
- Other `Tc2_Math` functions (`FLOOR`, `CEIL`, …) — added when needed,
  each via this same clean-room process.

## References

Public Beckhoff InfoSys documentation (also recorded in the library's
manifest `references`):

- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68446347.html> — `LTRUNC`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68444811.html> — `LMOD`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68447883.html> — `MODABS`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68443275.html> — `FRAC`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68440331.html> — `Tc2_Math` functions overview

No vendor implementation source, exported `.library` files, or encumbered
third-party source was used as an input to this spec or to the
implementation generated from it.
