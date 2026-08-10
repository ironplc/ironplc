# Library Interface Spec: `BOOL_TO_STRING`

This is a **clean-room interface/behavior spec** authored from the public
Beckhoff InfoSys documentation listed under *References*, per the
[Compatibility Library Authoring policy](../../steering/compatibility-library-authoring.md).
It is committed and merged as its own non-squashed git entry *before* any
implementation, and is the sole input to the implementation (Tier A —
the two-way mapping `TRUE ↔ 'TRUE'`, `FALSE ↔ 'FALSE'` is a fact with
essentially one expression).

IronPLC is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by any third party.

## Scope and placement

In TwinCAT, `BOOL_TO_STRING` is one of the implicit `BOOL_TO_*` conversion
operators — it belongs to the compiler surface, not to any vendor library,
and is therefore always in scope in a TwinCAT project. IronPLC does not add
vendor names to its compiler tables
([ADR-0042](../../adrs/0042-library-functions-over-compiler-intrinsics.md)
rule 1), so it ships as an ST function in the bundled **`Tc2_BuiltIns`**
library — the library that mirrors TwinCAT's built-in operator surface.

Because these operators are unconditionally available in the environment a
`.plcproj` targets, discovering a `.plcproj` **auto-activates**
`Tc2_BuiltIns` (no real `.plcproj` ever references it — there is nothing to
reference; the operators belong to no library there). That preserves the
portability promise in both directions: TwinCAT code that uses
`BOOL_TO_STRING` compiles in IronPLC unchanged, and code that compiles in
IronPLC against a TwinCAT project works in TwinCAT. This is not
source-sniffing — the `.plcproj` itself is the project's explicit statement
of the TwinCAT target (the design's "never sniff, never guess" rule forbids
inferring a library from POU *source content*). The library can also be
activated explicitly via `--library Tc2_BuiltIns` for source with no
project context.

## Interface

```iecst
FUNCTION BOOL_TO_STRING : STRING
VAR_INPUT
    IN : BOOL;
END_VAR
```

## Behavior

Converts a `BOOL` to its string representation:

| Call | Result |
|---|---|
| `BOOL_TO_STRING(TRUE)` | `'TRUE'` |
| `BOOL_TO_STRING(FALSE)` | `'FALSE'` |

The result is exactly the uppercase keyword spelling, with no padding.

**Medium:** trivially expressible, so it is an ST body in the library — no
intrinsic, no func_id, no wire-format commitment:

```iecst
IF IN THEN
    BOOL_TO_STRING := 'TRUE';
ELSE
    BOOL_TO_STRING := 'FALSE';
END_IF;
```

## References

Public Beckhoff InfoSys documentation (also recorded in the library's
manifest `references`):

- <https://infosys.beckhoff.com/content/1033/tcplccontrol/925577611.html> — `BOOL_TO` conversions (result is `'TRUE'` / `'FALSE'`)
- <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529047435.html> — Boolean conversion (TwinCAT 3)

No vendor implementation source, exported `.library` files, or encumbered
third-party source was used as an input to this spec or to the
implementation generated from it.
