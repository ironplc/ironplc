# Behavior Specification: `Tc2_Math` Compatibility Library

This document is the **clean-room behavior specification** for IronPLC's
`Tc2_Math` compatibility library. It was authored from the public Beckhoff
InfoSys documentation listed under *References*, per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md),
and it is the **sole input** to the implementation: the implementation PR
must be writable from this document alone, consulting no vendor material.
No vendor implementation source, exported `.library` files, or encumbered
third-party source was used as an input to this specification.

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party. The `Tc2_Math` name is used
nominatively: it records whose interface this library mirrors.

## Scope

Four functions mirroring Beckhoff's TwinCAT 3 `Tc2_Math` PLC library:
`LTRUNC`, `LMOD`, `MODABS`, and `FRAC`. These resolve the stalled
contributions in [#1217](https://github.com/ironplc/ironplc/pull/1217)
(`LTRUNC`/`LMOD`) and
[#1218](https://github.com/ironplc/ironplc/pull/1218) (`MODABS`) the way
review directed — as a compatibility library, not compiler intrinsics —
plus `FRAC`, requested in the same review. Other `Tc2_Math` functions
(`FLOOR`, `CEIL`, …) are out of scope and follow this same process when
added.

## Library identity and activation

| Property | Value |
|---|---|
| Package name | `Tc2_Math` (directory `compiler/sources/resources/libs/Tc2_Math/`) |
| Manifest `vendor` | `Beckhoff Automation GmbH` (nominative) |
| Version | `1.0.0` (`default_version`) |
| Activation | **Reference-activated only**: a `.plcproj` `<PlaceholderReference Include="Tc2_Math">` / `<LibraryReference>` entry, or `--library Tc2_Math`. |

`Tc2_Math` is **not** implicit: unlike TwinCAT's built-in operators
(`Tc2_BuiltIns`), real TwinCAT projects state their `Tc2_Math` dependency
in the project file, so IronPLC activates it exactly when a real project
would have it in scope. The manifest carries **no bindings table**: every
function is a fully-defined ST body.

## Data types

All four signatures are **`LREAL`-only**, matching Beckhoff (whose
`Tc2_Math` signatures are not generic). A call with `REAL` arguments is
not this library's concern: TwinCAT accepts it by implicitly widening the
argument REAL→LREAL at the call site, which is separately planned work
([2026-07-27-twincat-real-to-lreal-widening.md](../../plans/2026-07-27-twincat-real-to-lreal-widening.md))
and serves every LREAL-taking function uniformly.

Formal parameter names are normalized to IronPLC's stdlib `IN`/`IN1`/`IN2`
convention, consistent with how IronPLC records every other function
signature; real call sites use positional arguments for these functions.

## Implementation medium

Every function is an **ST body** in the library's `.st` file — no VM
builtin, no func_id, no manifest binding. The two semantics IEC 61131-3
source cannot express (real-preserving truncation, floating modulo) are
reached through the typed compiler intrinsics `__TRUNC(ANY_REAL): ANY_REAL`
and `__MOD(IN1, IN2: ANY_REAL): ANY_REAL`
(merged in [#1348](https://github.com/ironplc/ironplc/pull/1348); see
`specs/plans/2026-08-11-compiler-intrinsic-trunc-mod.md`), which library
bodies may call. `MODABS` and `FRAC` are math-dictated compositions.

The intrinsics obey IEEE-754 double-precision (binary64) arithmetic. All
edge-case behavior below follows from that and is **normative**: the
implementation must produce these results, and the end-to-end tests must
pin them.

---

## `LTRUNC` — integer part, staying LREAL

### Signature

```iecst
FUNCTION LTRUNC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
```

### Behavior (normative)

Returns the integer part of `IN`: the fractional digits are discarded,
rounding **toward zero**. For positive inputs the result is ≤ `IN`; for
negative inputs it is ≥ `IN`. Unlike the IEC 61131-3 `TRUNC` (whose result
is `ANY_INT`), the result is `LREAL` and is therefore **not limited to any
integer type's value range**: inputs whose magnitude exceeds `LINT` range
truncate exactly, without clamping, overflow, or precision loss beyond
that inherent in binary64.

Definition: `LTRUNC(IN) = __TRUNC(IN)`.

### Edge cases (normative)

| Input | Result |
|---|---|
| `+0.0` / `-0.0` | `+0.0` / `-0.0` (sign of zero preserved) |
| `x` with `|x| < 1.0` | `±0.0` (sign of `x`) |
| `+∞` / `-∞` | `+∞` / `-∞` |
| `NaN` | `NaN` |
| `|x| ≥ 2^52` | `x` unchanged (already integral in binary64) |

### Test vectors (exact equality)

| Call | Result |
|---|---|
| `LTRUNC(2.8)` | `2.0` |
| `LTRUNC(-2.8)` | `-2.0` |
| `LTRUNC(3.7)` | `3.0` |
| `LTRUNC(-3.7)` | `-3.0` |
| `LTRUNC(5.0)` | `5.0` |
| `LTRUNC(0.9)` | `0.0` |
| `LTRUNC(-0.9)` | `-0.0` |
| `LTRUNC(1.5E300)` | `1.5E300` (no clamping) |
| `LTRUNC(9.3E18)` | `9.3E18` (beyond LINT range, exact) |

---

## `LMOD` — floating modulo, signed remainder

### Signature

```iecst
FUNCTION LMOD : LREAL
VAR_INPUT
    IN1 : LREAL;    (* value *)
    IN2 : LREAL;    (* modulo range *)
END_VAR
```

### Behavior (normative)

Modulo division returning the **signed** remainder. Unlike the
integer-only IEC `MOD`, `LMOD` operates on floating-point values and
returns non-integer remainders. The semantics are IEEE-754 `fmod`
(C `fmod`, Rust `%` on `f64`):

```
LMOD(IN1, IN2) = IN1 − LTRUNC(IN1 / IN2) × IN2      (finite, nonzero IN2)
```

computed exactly (the true `fmod`, not the literal expression above, which
would round `IN1 / IN2`). Properties:

- The result has the **sign of `IN1`** (the dividend), or is zero.
- `|result| < |IN2|`.
- The result is exact — no rounding error is introduced by the operation.

Definition: `LMOD(IN1, IN2) = __MOD(IN1, IN2)`.

### Edge cases (normative)

| Input | Result |
|---|---|
| `LMOD(x, 0.0)` | `NaN` — **never** a runtime error or trap |
| `LMOD(±0.0, y)`, `y ≠ 0` | `±0.0` (sign of `IN1`) |
| `LMOD(±∞, y)` | `NaN` |
| `LMOD(x, ±∞)`, finite `x` | `x` |
| `NaN` in either operand | `NaN` |
| `IN2 < 0` | same magnitude as `LMOD(IN1, |IN2|)`; sign still follows `IN1` |

### Test vectors

Approximate comparisons use `|actual − expected| < 1.0E-9`.

| Call | Result |
|---|---|
| `LMOD(400.56, 360.0)` | `≈ 40.56` |
| `LMOD(-400.56, 360.0)` | `≈ -40.56` |
| `LMOD(400.56, -360.0)` | `≈ 40.56` (sign of dividend) |
| `LMOD(7.0, 3.5)` | `0.0` (exact) |
| `LMOD(1.5, 0.0)` | `NaN` |

---

## `MODABS` — modulo, unsigned result in the modulo range

### Signature

```iecst
FUNCTION MODABS : LREAL
VAR_INPUT
    IN : LREAL;     (* value *)
    IM : LREAL;     (* modulo range *)
END_VAR
```

### Behavior (normative)

Modulo division returning the **unsigned** modulo value within the modulo
range: the result lies in `[0.0, |IM|)`. This is the wrap-around used for
positioning — e.g. normalizing an angle into `[0°, 360°)` — and is the
`Tc2_Math` counterpart of `LMOD` for applications that need a
non-negative representative.

Math-dictated definition in terms of `LMOD`:

```
r = LMOD(IN, IM)
MODABS(IN, IM) = r            when r ≥ 0
               = r + |IM|     when r < 0
```

### Edge cases (normative)

| Input | Result |
|---|---|
| `MODABS(x, 0.0)` | `NaN` (propagated from `LMOD`) |
| `MODABS(±0.0, y)`, `y ≠ 0` | `0.0` |
| `IM < 0` | identical to `MODABS(IN, |IM|)` — the range is `[0, |IM|)` |
| `MODABS(±∞, y)` | `NaN` |
| `NaN` in either operand | `NaN` |

### Test vectors

| Call | Result |
|---|---|
| `MODABS(400.56, 360.0)` | `≈ 40.56` |
| `MODABS(-400.56, 360.0)` | `≈ 319.44` |
| `MODABS(-400.56, -360.0)` | `≈ 319.44` (negative `IM` uses its magnitude) |
| `MODABS(720.0, 360.0)` | `0.0` (exact) |
| `MODABS(-360.0, 360.0)` | `0.0` (exact — a result of exactly `|IM|` must not appear) |
| `MODABS(1.5, 0.0)` | `NaN` |

> Implementation note: the two-branch definition above never produces
> `|IM|` itself. A naive `LMOD(IN, IM) + |IM|` applied unconditionally
> would, for `LMOD = -0.0` or `0`, return `|IM|`, which is **outside** the
> range — the conditional is required.

---

## `FRAC` — fractional part

### Signature

```iecst
FUNCTION FRAC : LREAL
VAR_INPUT
    IN : LREAL;
END_VAR
```

### Behavior (normative)

Returns the decimal (fractional) component of `IN`. The result keeps the
**sign of the input** and satisfies `|result| < 1.0`.

Math-dictated definition in terms of `LTRUNC`:

```
FRAC(IN) = IN − LTRUNC(IN)
```

### Edge cases (normative)

| Input | Result |
|---|---|
| integral `x` (incl. `±0.0`) | `±0.0` (sign of `x`) |
| `±∞` | `NaN` (`∞ − ∞`) |
| `NaN` | `NaN` |
| `|x| ≥ 2^52` | `0.0` (binary64 has no fractional digits there) |

### Test vectors

| Call | Result |
|---|---|
| `FRAC(2.8)` | `≈ 0.8` |
| `FRAC(-2.8)` | `≈ -0.8` |
| `FRAC(3.7)` | `≈ 0.7` |
| `FRAC(-3.7)` | `≈ -0.7` |
| `FRAC(5.0)` | `0.0` (exact) |

---

## Package contents (normative for the implementation PR)

```
compiler/sources/resources/libs/Tc2_Math/
├── library.toml
└── 1.0.0/
    └── Tc2_Math.st
```

- `library.toml`: `name = "Tc2_Math"`, `vendor = "Beckhoff Automation
  GmbH"`, `default_version = "1.0.0"`, and a non-empty `references` list
  containing the public URLs from *References* below. No bindings table.
- `Tc2_Math.st`: the four functions as fully-defined ST bodies
  implementing the definitions above (calling `__TRUNC`/`__MOD`; `MODABS`
  and `FRAC` composed per their math-dictated definitions), with a header
  comment carrying the non-affiliation statement and a pointer to this
  specification.

## Acceptance criteria for the implementation PR

1. Loader: the bundled registry contains and loads `Tc2_Math`.
2. Every test vector above is pinned by an end-to-end test (activate →
   analyze → codegen → VM run), using exact equality where the table says
   exact and `1.0E-9` epsilon otherwise. NaN rows assert `IS NaN`, never a
   trap.
3. A `.plcproj`-driven activation test: a project referencing `Tc2_Math`
   (no `--library` flag) compiles source calling all four functions — and
   the same source **fails** `check` without the reference (dormant by
   default).
4. Shadowing: a user-defined function named `LTRUNC` takes precedence over
   the library's.
5. `plc2plc` round-trips user source calling these functions unchanged.
6. Full CI (`cd compiler && just`) green.

## References

Public Beckhoff InfoSys documentation (to be recorded in the library's
manifest `references`):

- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68446347.html> — `LTRUNC`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68444811.html> — `LMOD`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68447883.html> — `MODABS`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68443275.html> — `FRAC`
- <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68440331.html> — `Tc2_Math` functions overview

The public documentation defines these functions' behavior informally
(descriptions and examples such as `LMOD(400.56, 360) = 40.56`,
`MODABS(-400.56, 360) = 319.44`, `FRAC(2.8) = 0.8`). The precise IEEE-754
edge-case semantics above (NaN/infinity handling, `x % 0.0 = NaN`,
sign-of-dividend, sign-of-zero) are **IronPLC's own normative choices**,
selected to match standard `fmod`/`trunc` behavior; they are pinned here
so the implementation and its tests have a single authority.
