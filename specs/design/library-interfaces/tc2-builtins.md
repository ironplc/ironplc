# Library Interface Spec: Tc2_BuiltIns

Clean-room interface/behavior spec for the bundled `Tc2_BuiltIns`
compatibility library, authored per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md).
This commit is the durable record of *what was authored from what*: it precedes
the implementation and is merged as its own non-squashed git entry.

## What this library models

TwinCAT 3 makes a set of names available to **every** PLC project with no
library reference in the `.plcproj` — the built-in (compiler-operator) surface.
Beckhoff documents these as *operators* in the TwinCAT 3 PLC introduction
(`tc3_plc_intro`), not in any library manual, and states that operators are
implicitly known throughout a project. `Tc2_BuiltIns` is IronPLC's handle for
that implicit surface, so that:

- a TwinCAT project using these names compiles in IronPLC with no edits and no
  stated reference (the library activates implicitly on `.plcproj` discovery),
  and
- code written in IronPLC against this library compiles unchanged in TwinCAT,
  because TwinCAT always provides these names.

The library ships only names that are **not** part of the IEC 61131-3 standard
surface the compiler already seeds (per
[ADR-0042](../../adrs/0042-library-functions-over-compiler-intrinsics.md),
vendor-defined names are always library functions).

## Allowed inputs used

Public Beckhoff InfoSys documentation only (behavior, names, signatures — no
vendor implementation source, no exported `.library` files, no IEC standard
text):

- TwinCAT 3 PLC: Boolean conversion operators —
  `https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529047435.html`
- TwinCAT 3 PLC: Type conversion operators (overview) —
  `https://infosys.beckhoff.com/content/1033/tc3_plc_intro/3998090635.html`
- TwinCAT 3 PLC: Operators (implicitly known throughout the project) —
  `https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528853899.html`

## Interface and behavior

### `BOOL_TO_STRING`

```
FUNCTION BOOL_TO_STRING : STRING
VAR_INPUT
    IN : BOOL;
END_VAR
```

Behavior (dictated by the documented conversion result — the body has
essentially one expression, Tier A/B under the authoring policy):

| Input | Result |
|-------|--------|
| `TRUE` | `'TRUE'` |
| `FALSE` | `'FALSE'` |

Notes:

- The result is the five-character/four-character uppercase word, not `'1'` /
  `'0'` (the numeric forms apply to `BOOL_TO_<number>` conversions, per the
  Boolean-conversion documentation).
- The result type is the default `STRING`; no truncation concerns arise
  (maximum result length 5).
- The body is IronPLC-authored ST (`IF IN THEN … 'TRUE' … ELSE … 'FALSE'`),
  a different medium than a compiler operator, so the output structurally
  cannot copy a vendor source expression.

## Non-affiliation

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party. The `vendor` manifest field is
nominative — it records whose interface this library mirrors.

## Clearance

No licensed copy of a vendor implementation was available or consulted; the
behavior table above derives solely from the public documentation listed under
*Allowed inputs used*.
