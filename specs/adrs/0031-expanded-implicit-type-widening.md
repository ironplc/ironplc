# Expanded Implicit Type Widening

status: accepted
date: 2026-04-03
amended: 2026-09-12 (UDINT/DWORD equal-width exception recorded; status unchanged)
amended: 2026-09-13 (the single cross-family flag split into three, one per
rule; status unchanged — see the amendment note at the end)
amended: 2026-09-21 (REAL → LREAL widening recorded; status unchanged — see the
amendment note at the end)

## Context and Problem Statement

ADR-0029 introduced implicit integer widening (e.g. SINT → INT → DINT → LINT) but explicitly excluded bit-string types, REAL/LREAL, and cross-family conversions. This leaves several common patterns unsupported that work in RuSTy, CODESYS, and TwinCAT:

1. Passing an INT variable to a REAL parameter (integer → real)
2. Passing a BYTE variable to a WORD parameter (bit-string widening)
3. Passing a BYTE variable to an INT parameter (cross-family)
4. Passing a bare integer literal `0` to a BYTE parameter (cross-family literal)
5. Assigning a BYTE return value to an INT variable (cross-family return)

Cases 1 and 2 fall within the IEC 61131-3 type hierarchy (ANY_NUM and ANY_BIT respectively). Cases 3–5 cross from ANY_BIT to ANY_INT, which is outside the standard's implicit widening rules.

## Decision Drivers

* **IEC 61131-3 compliance** — the standard's type hierarchy places ANY_INT and ANY_REAL under ANY_NUM, and defines BYTE/WORD/DWORD/LWORD under ANY_BIT, implying widening within each branch
* **Safety** — lossless integer-to-real widening preserves all values; bit-string widening is zero-extension
* **Industry alignment** — CODESYS, TwinCAT, and RuSTy support all five cases
* **Practical impact** — OSCAT and similar libraries rely on these patterns

## Considered Options

* Keep current restrictions — users must write explicit conversion calls
* Allow standard-compliant widening by default, gate cross-family behind a flag

## Decision Outcome

Chosen option: "Allow standard-compliant widening by default, gate cross-family behind `--allow-cross-family-widening`."

### Standard Widening (enabled by default)

#### Integer → REAL/LREAL (lossless only)

An integer type can implicitly widen to a real type when all values of the source type are exactly representable in the target type:

* SINT(8), INT(16), USINT(8), UINT(16) → REAL (32-bit float, 23-bit mantissa)
* Any integer type → LREAL (64-bit float, 52-bit mantissa)

Not allowed (lossy): DINT(32), LINT(64), UDINT(32), ULINT(64) → REAL. Use explicit conversion (e.g. `DINT_TO_REAL(x)`) or widen to LREAL.

#### REAL → LREAL (lossless)

A REAL value can implicitly widen to LREAL. Every 32-bit float is exactly
representable in 64-bit, so the conversion never loses a value.

Not allowed (lossy): LREAL(64) → REAL(32). Use explicit conversion
(`LREAL_TO_REAL(x)`).

This arm was not part of the original decision. It shipped later and is
recorded by *Amendment: REAL widens to LREAL*.

#### Bit-string widening

Wider bit-string types can accept narrower bit-string values:

* BYTE(8) → WORD(16) → DWORD(32) → LWORD(64)

BOOL is excluded. While IEC 61131-3 places BOOL under ANY_BIT, it is semantically a boolean (TRUE/FALSE), not a numeric bit container. Standard implementations do not define BOOL → BYTE as implicit widening.

### Cross-Family Widening (requires `--allow-cross-family-widening`)

These conversions cross the ANY_BIT / ANY_INT boundary and are not part of the IEC 61131-3 standard:

* Bit-string → integer: BYTE → INT, WORD → DINT, etc. (target must be strictly wider, with one exception — see *Amendment: UDINT and DWORD widen in both directions at equal width*)
* Bare integer literal → bit-string: `0` where BYTE is expected
* Return type: function returning BYTE assigned to INT variable

These three rules are gated by three flags — `--allow-cross-family-widening`,
`--allow-cross-family-conversion` and `--allow-int-literal-to-bit-string` —
each enabled by default in the `Rusty`, `CODESYS` and `TwinCAT` dialects. See
the amendment at the end for why they are three.

### Scope

This applies to:

* **Function arguments (P4026)** — argument type compatible with parameter type
* **Function return types (P4027)** — return type compatible with assignment target

### Consequences

* Good, because standard-compliant widening works without any flags
* Good, because cross-family widening is available for RuSTy compatibility
* Good, because the flag makes the non-standard behavior explicit
* Good, because all standard widening conversions are lossless
* Neutral, because explicit conversion functions remain available and recommended

## Relationship to Prior ADRs

* **ADR-0047** (exact type matching): Still applies for non-widening cases
* **ADR-0028** (literal type inference): Bare literal → REAL/LREAL remains as-is; bare literal → ANY_BIT is new and gated
* **ADR-0029** (integer widening): Extended to include integer → real (lossless), real → real (REAL → LREAL, added by amendment) and bit-string widening within ANY_BIT

## Amendment: UDINT and DWORD widen in both directions at equal width (2026-09-12)

This ADR states one cross-family rule — bit-string to integer, target strictly
wider — and the compiler has shipped a second one since the UDINT/DWORD work
landed. `ElementaryTypeName::can_widen_cross_family_to`
(`compiler/dsl/src/common.rs:1001-1019`) allows `UDINT` and `DWORD` to convert
implicitly in *both* directions under `--allow-cross-family-widening`, despite
the two being equal width. That also makes it the one case where integer to
bit-string is implicit, a direction this ADR does not mention at all.

The exception is deliberate and evidence-backed. Beckhoff's own documentation
states no implicit conversion exists between bit-string and integer types even
at equal width, but a real TcXaeShell build accepted it, so IronPLC follows the
implementation rather than the documentation. The rationale, including the
scoping argument, lives in the doc comment at `common.rs:985-1000`.

Measured on this tree with the flag enabled (`udValue : UDINT := 3000000000`,
`dwValue : DWORD := 16#FFFFFFFF`):

| Conversion | Direction | Result |
|---|---|---|
| `dwFromUdint := udValue` | integer → bit-string | `16#B2D05E00` (= 3000000000) |
| `udFromDword := dwValue` | bit-string → integer | `4294967295` |

Both are correct because the two types share a 32-bit slot, so the conversion is
a bit-pattern no-op. The "UDINT at or above 2^31 reinterprets as a negative i32"
hazard does not reach this path.

The exception is scoped to exactly this pair, and nothing wider should be
inferred from it. The other equal-width bit-string/unsigned-integer pairs
(`BYTE`/`USINT`, `WORD`/`UINT`, `LWORD`/`ULINT`) and all signed integers are
unverified and stay rejected: with the flag on, `WORD` ↔ `UINT` still reports
P4035 in both directions. Without the flag, `UDINT` ↔ `DWORD` reports P4035 too.

Two tests pin this. `common.rs` covers the predicate in both directions
alongside the rejected `WORD` ↔ `UINT` control; `udint_arg_to_dword_param_ok` in
`compiler/analyzer/src/rule_function_call_type_check.rs` covers the
integer → bit-string direction through the function-call rule, next to the
`INT` → `BYTE` case that must stay an error.

The TcXaeShell POU that established the exception is not recoverable: the
write-up that recorded it (dated 2026-07-27) is not in this repository's
history at all, so the summary in `common.rs` is the whole of the surviving
evidence. Extending the exception to the other equal-width pairs therefore
means gathering that evidence again, not reasoning outward from this pair.

This amendment corrects the record; the decision is unchanged. ADR-0031 was
also missing the REAL → LREAL arm when this amendment was written; that arm is
now enumerated under Standard Widening and recorded by *Amendment: REAL widens
to LREAL*.

## Amendment: one flag became three, one per rule (2026-09-13)

`--allow-cross-family-widening` gated three rules and named one of them:

| Rule | Widens? | Now gated by |
|---|---|---|
| bit-string → strictly wider integer (`BYTE` → `INT`) | yes | `--allow-cross-family-widening` |
| `UDINT` ↔ `DWORD` at equal width, both directions | no — a reinterpretation of the same 32 bits | `--allow-cross-family-conversion` |
| bare integer literal where a bit-string is expected (`0` → `BYTE`) | no — literal typing, in the sense of ADR-0028 | `--allow-int-literal-to-bit-string` |

Widening moves a value into a type that can hold strictly more. The equal-width
rule moves nothing and runs in both directions, and literal typing is a third
thing again. One name covering all three masked two of them: a user reading
"widening" could not tell they were also enabling a bidirectional conversion.

The predicate split the same way. `ElementaryTypeName::can_widen_cross_family_to`
now answers only the strictly-wider question; `can_convert_cross_family_to` is
the equal-width rule, scoped to exactly the verified pair as before. Each of
the three sites in `compiler/analyzer/src/type_compat.rs` checks its own flag,
and `apply_when_one_cross_family_flag_on_then_only_its_rule_is_accepted` in
`rule_function_call_type_check.rs` pins the 3×3 grid: each program is accepted
under exactly one flag and rejected under the other two and under none.

All three flags are enabled in exactly the dialects the single flag was —
`Rusty`, `CODESYS` and `TwinCAT` — so no dialect's behaviour changed. This
amendment corrects the model, not the decision. Whether the conversion flag
should be narrowed to the dialect its evidence came from is a separate
decision, deliberately not taken here.

This is a breaking change for anyone passing `--allow-cross-family-widening`
explicitly and relying on it for `UDINT` ↔ `DWORD` or for bare literals: those
now need the corresponding flag as well. Dialect presets are unaffected.

## Amendment: REAL widens to LREAL (2026-09-21)

This ADR's Standard Widening section enumerated two arms — integer → real and
bit-string — and the compiler has shipped a third since #1245 landed
(`03a7c855`, 2026-07-28, roughly four months after this decision).
`ElementaryTypeName::can_widen_to` carries a `(Real, Real)` arm allowing a
narrower real to widen to a wider one, which for the two real types IronPLC has
means exactly REAL → LREAL.

It belongs in Standard Widening, by this ADR's own test: IEC 61131-3 places
REAL and LREAL under ANY_REAL, and every 32-bit float is exactly representable
in 64 bits, so the source type's full value range survives. The reverse
direction is narrowing and stays rejected.

The omission was easy to miss and hard to see through. ADR-0029 explicitly
excluded REAL/LREAL, this ADR's Context lists five motivating cases and none of
them is real → real, and the arm arrived in a commit titled `feat(twincat)`.
That title describes the motivating case — a Beckhoff library function
declaring an LREAL parameter, called with a typed REAL variable — not the
scope. The arm went into `can_widen_to`, which no flag guards, so it applies in
every dialect including plain `iec61131-3`. A reader taking ADR-0029 and this
ADR together would have concluded an explicit `REAL_TO_LREAL` call was still
required.

Measured on this tree with no flags and no `--dialect` (`r : REAL := 1.5`,
`l : LREAL`, `GivesReal`/`TakesLreal` returning and taking REAL/LREAL):

| Statement | Conversion | Result |
|---|---|---|
| `l := r;` | REAL variable → LREAL variable | accepted |
| `l := TakesLreal(r);` | REAL argument → LREAL parameter | accepted |
| `l := GivesReal(r);` | REAL return → LREAL variable | accepted |
| `r := l;` | LREAL variable → REAL variable | `P4035` |
| `r := TakesReal(l);` | LREAL argument → REAL parameter | `P4026` |

Three tests pin this. `common.rs` covers the predicate in both directions
(`real_to_lreal` accepted, `lreal_to_real` rejected), and
`apply_when_typed_real_var_arg_to_lreal_param_then_ok` and
`apply_when_typed_lreal_var_arg_to_real_param_then_error` in
`compiler/analyzer/src/rule_function_call_type_check.rs` cover both directions
through the function-call rule, next to the bare-literal case that has always
worked by the separate path of ADR-0028.

The user documentation already described the arm (`docs/explanation/type-conversions.rst`,
"Real widening"), though its introduction still counted three categories of
widening while listing four below; that count is corrected alongside this
amendment. This amendment corrects the record; the behaviour is unchanged.
