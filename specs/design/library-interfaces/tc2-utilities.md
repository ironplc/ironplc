# Behavior Specification: `Tc2_Utilities` Compatibility Library

This document is the **clean-room behavior specification** for IronPLC's
`Tc2_Utilities` compatibility library. It was authored from the public
Beckhoff InfoSys documentation listed under *References*, per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md),
and it is the **sole input** to the implementation: the implementation PR
must be writable from this document alone, consulting no vendor material.
No vendor implementation source, exported `.library` files, or encumbered
third-party source was used as an input to this specification.

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party. The `Tc2_Utilities` name is
used nominatively: it records whose interface this library mirrors.

## Scope

One function mirroring Beckhoff's TwinCAT 3 `Tc2_Utilities` PLC library:
`LREAL_TO_FMTSTR`. This resolves the `LREAL_TO_FMTSTR` portion of the
stalled contribution in
[#1246](https://github.com/ironplc/ironplc/pull/1246) the way review
directed — as a compatibility library, not a flag-gated stdlib
registration. Other `Tc2_Utilities` functions are out of scope and follow
this same process when added.

`LREAL_TO_FMTSTR` is **not** a general format-string engine. It is a
single, narrow formatter: fixed-point decimal notation with an adjustable
number of decimal places and a round-or-truncate switch. Beckhoff's
`%`-style formatting facility (`FB_FormatString`) is a different POU and
is not specified here.

## Library identity and activation

| Property | Value |
|---|---|
| Package name | `Tc2_Utilities` (directory `compiler/sources/resources/libs/Tc2_Utilities/`) |
| Manifest `vendor` | `Beckhoff Automation GmbH` (nominative) |
| Version | `1.0.0` (`default_version`) |
| Activation | **Reference-activated only**: a `.plcproj` `<PlaceholderReference Include="Tc2_Utilities">` / `<LibraryReference>` entry, or `--library Tc2_Utilities`. |

`Tc2_Utilities` is **not** implicit: real TwinCAT projects state the
dependency in the project file (the
[compatibility-libraries design appendix](../compatibility-libraries.md)
shows a real-world pinned `LibraryReference` for exactly this library), so
IronPLC activates it exactly when a real project would have it in scope.
The manifest carries **no bindings table**: the function is a
fully-defined ST body.

## Implementation medium

The function is an **ST body** in the library's `.st` file — no VM
builtin, no func_id, no manifest binding. The body may call the typed
compiler intrinsic `__TRUNC(ANY_REAL): ANY_REAL` (merged in
[#1348](https://github.com/ironplc/ironplc/pull/1348)), the standard
string functions (`CONCAT`, `MID`), and standard conversions
(`LREAL_TO_LINT`, which truncates toward zero). All floating-point steps
obey IEEE-754 double-precision (binary64) arithmetic; all digit
production is exact 64-bit integer arithmetic. The edge-case behavior
below follows from that and is **normative**: the implementation must
produce these results, and the end-to-end tests must pin them.

---

## `LREAL_TO_FMTSTR` — fixed-point decimal formatting

### Signature

```iecst
FUNCTION LREAL_TO_FMTSTR : STRING
VAR_INPUT
    in         : LREAL;   (* value to format *)
    iPrecision : INT;     (* number of decimal places *)
    bRound     : BOOL;    (* TRUE: round at the last place; FALSE: truncate *)
END_VAR
```

Formal parameter names are the vendor's documented names (`in`,
`iPrecision`, `bRound`), **not** normalized to IronPLC's `IN`/`IN1`/`IN2`
convention, so that vendor source using formal (named) argument passing
resolves unchanged. This intentionally differs from the
[`Tc2_Math` spec](tc2-math.md), whose homogeneous one/two-argument math
signatures are realistically only called positionally.

The vendor's nominal return type is `T_MaxString` (a `Tc2_Utilities`
alias for a maximum-length `STRING`). IronPLC declares plain `STRING`
(default capacity 254): the longest possible result under this
specification is 36 characters (sign + 19 integer digits + point + 15
fraction digits), so no truncation can occur. This is a recorded
simplification, not a behavioral difference.

### Behavior (normative)

Converts `in` to its fixed-point decimal string representation with
exactly `p` digits after the decimal point, where
`p = LIMIT(0, iPrecision, 15)`:

1. **Domain check.** If `in` is NaN, ±∞, or `|in| ≥ 2^63`
   (≈ 9.223 × 10^18), the result is the empty string `''` (see *Domain*
   below).
2. **Split.** `v = ABS(in)`; `I = __TRUNC(v)` (the integer part, exact in
   binary64); `f = v − I` (the fractional part — this subtraction is
   exact in binary64).
3. **Scale.** `s = f × 10^p`. `10^p` is exactly representable in binary64
   for `p ≤ 15`, so this multiplication incurs **exactly one** binary64
   rounding — the only one in the whole computation.
4. **Round or truncate.** If `bRound` then `n = trunc(s + 0.5)` (round
   half **away from zero** — because step 2 works on `|in|`, negative
   inputs round symmetrically), else `n = trunc(s)` (truncate toward
   zero). Because `s < 10^15 < 2^52`, the addition of `0.5` and the
   truncation are exact.
5. **Carry.** If `n = 10^p` (the fraction rounded all the way up, e.g.
   `0.996` at `p = 2`), then `I := I + 1` and `n := 0`.
6. **Render.** The result is the concatenation of:
   - `'-'` if `in < 0.0` (note: `-0.0 < 0.0` is FALSE, so negative zero
     renders unsigned),
   - the exact decimal rendering of `I` (at least one digit — a zero
     integer part renders `'0'`),
   - if `p > 0`: `'.'` followed by the decimal rendering of `n`,
     **left-padded with zeros to exactly `p` digits**.
   When `p = 0` there is no decimal point. There are no thousands
   separators, no `'+'` sign, no exponent notation, and the decimal
   separator is always `'.'`.

Steps 2, 5, and 6 use exact arithmetic (binary64 below 2^53 and 64-bit
integers), so every digit of the result is the exact decimal rendering of
the computed value; the single rounding in step 3 is the only place the
result can differ from an infinitely-precise formatter, and only in the
last decimal place for values within ~1 ulp of a rounding boundary. This
is the same computation shape a fixed-point ST formatter necessarily has,
and it is deterministic and pinned by the test vectors below.

### Domain (normative)

| Region | Result | Rationale |
|---|---|---|
| finite `in`, `|in| < 2^63` | formatted string per above | integer part fits `LINT` exactly; binary64 values ≥ 2^52 are already integral, so their fraction digits are genuinely all zeros |
| `|in| ≥ 2^63` | `''` | a faithful fixed-point rendering would require arbitrary-precision arithmetic; per the [Safety first](../compatibility-libraries.md) principle the library returns a *detectably* out-of-domain result rather than a silently wrong one |
| `+∞` / `-∞` | `''` | no fixed-point rendering exists |
| NaN | `''` | ditto; NaN fails every comparison, so the implementation's domain check must test it deliberately (`in = in` is FALSE for NaN) |
| `iPrecision < 0` | treated as `0` | clamped, not an error |
| `iPrecision > 15` | treated as `15` | `10^15 < 2^52` keeps steps 3–4 exact; binary64 carries at most ~17 significant decimal digits, so further places would be noise |

The vendor's public page describes the function informally ("converts a
floating-point value into a string with a selectable number of decimal
places, optionally rounding at the last place") and does not document
range limits, the rounding mode, or non-finite behavior. As with the
`Tc2_Math` spec, the precise semantics above are **IronPLC's own
normative choices**, pinned here so the implementation and its tests have
a single authority. Where TwinCAT's observable behavior is later found to
differ (see *Clearance*), reconciling is a spec change first.

### Test vectors (exact string equality)

Sign, rounding mode, and truncation:

| Call | Result |
|---|---|
| `LREAL_TO_FMTSTR(123.456, 2, TRUE)` | `'123.46'` |
| `LREAL_TO_FMTSTR(123.456, 2, FALSE)` | `'123.45'` |
| `LREAL_TO_FMTSTR(-123.456, 2, TRUE)` | `'-123.46'` |
| `LREAL_TO_FMTSTR(2.5, 0, TRUE)` | `'3'` (half away from zero, not banker's) |
| `LREAL_TO_FMTSTR(-2.5, 0, TRUE)` | `'-3'` (symmetric) |
| `LREAL_TO_FMTSTR(2.5, 0, FALSE)` | `'2'` |
| `LREAL_TO_FMTSTR(-2.8, 0, FALSE)` | `'-2'` (truncation is toward zero) |

Carry, padding, and zero:

| Call | Result |
|---|---|
| `LREAL_TO_FMTSTR(0.996, 2, TRUE)` | `'1.00'` (carry into the integer part) |
| `LREAL_TO_FMTSTR(-0.996, 2, FALSE)` | `'-0.99'` |
| `LREAL_TO_FMTSTR(1.05, 2, TRUE)` | `'1.05'` (fraction left-padded: `n = 5` renders `'05'`) |
| `LREAL_TO_FMTSTR(1.5, 3, FALSE)` | `'1.500'` (fraction zero-filled to `p`) |
| `LREAL_TO_FMTSTR(0.0, 2, TRUE)` | `'0.00'` |
| `LREAL_TO_FMTSTR(-0.0, 2, TRUE)` | `'0.00'` (negative zero: no sign) |

Precision clamping:

| Call | Result |
|---|---|
| `LREAL_TO_FMTSTR(1.5, -3, TRUE)` | `'2'` (precision clamped to 0, then rounded) |
| `LREAL_TO_FMTSTR(1.5, 100, FALSE)` | `'1.500000000000000'` (clamped to 15) |

Domain boundaries:

| Call | Result |
|---|---|
| `LREAL_TO_FMTSTR(9007199254740993.0, 0, TRUE)` | `'9007199254740992'` (the literal is not representable; the *stored* binary64 value — 2^53 — renders exactly) |
| `LREAL_TO_FMTSTR(9.0E18, 0, TRUE)` | `'9000000000000000000'` (in domain; values ≥ 2^52 are integral, fraction digits genuinely zero) |
| `LREAL_TO_FMTSTR(1.0E19, 0, TRUE)` | `''` (out of domain) |
| `LREAL_TO_FMTSTR(-1.0E19, 2, TRUE)` | `''` |
| `in = +∞` or `-∞` (constructed arithmetically, e.g. `1.0E308 * 10.0`) | `''` |
| `in = NaN` (constructed arithmetically, e.g. `__MOD(1.5, 0.0)`) | `''` |

---

## Package contents (normative for the implementation PR)

```
compiler/sources/resources/libs/Tc2_Utilities/
├── library.toml
└── 1.0.0/
    └── Tc2_Utilities.st
```

- `library.toml`: `name = "Tc2_Utilities"`, `vendor = "Beckhoff
  Automation GmbH"`, `default_version = "1.0.0"`, and a non-empty
  `references` list containing the public URLs from *References* below.
  No bindings table.
- `Tc2_Utilities.st`: `LREAL_TO_FMTSTR` as a fully-defined ST body
  implementing the definition above, with a header comment carrying the
  non-affiliation statement and a pointer to this specification.

## Acceptance criteria for the implementation PR

1. Loader: the bundled registry contains and loads `Tc2_Utilities`.
2. Every test vector above is pinned by an end-to-end test (activate →
   analyze → codegen → VM run) asserting exact string equality.
3. A `.plcproj`-driven activation test: a project referencing
   `Tc2_Utilities` (no `--library` flag) compiles source calling
   `LREAL_TO_FMTSTR` — and the same source **fails** `check` without the
   reference (dormant by default).
4. Shadowing: a user-defined function named `LREAL_TO_FMTSTR` takes
   precedence over the library's.
5. `plc2plc` round-trips user source calling the function unchanged.
6. Full CI (`cd compiler && just`) green.

## Clearance

No licensed copy of a vendor implementation was available or consulted.
If a licensed TwinCAT installation becomes available, compare observed
outputs against the vectors above (comparison for clearance is not
copying) and record the result; any divergence is reconciled by amending
this specification first, then the implementation.

## References

Public Beckhoff InfoSys documentation (to be recorded in the library's
manifest `references`):

- Beckhoff InfoSys, TwinCAT 3 PLC Library `Tc2_Utilities`:
  `LREAL_TO_FMTSTR` function page (infosys.beckhoff.com, section
  *TwinCAT 3 → PLC libraries → Tc2_Utilities → Functions → Formatting*).
  The signature above (`in : LREAL`, `iPrecision : INT`,
  `bRound : BOOL`) matches the verification against this page recorded in
  [#1246](https://github.com/ironplc/ironplc/pull/1246).

> **Pin before merge:** `infosys.beckhoff.com` is unreachable from the
> authoring environment (network egress policy), so the exact page URL
> must be captured and substituted here and in `library.toml` before this
> spec merges. While pinning, confirm against the page: (a) the exact
> documented return type — #1246 records `STRING(510)`, while
> `T_MaxString` is conventionally `STRING(255)`; either way the plain
> `STRING` declaration above stands, but the record should be accurate —
> and (b) whether the page documents a precision limit, a rounding mode,
> or any result examples; documented examples should be added to the test
> vectors verbatim.
