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
operators — it belongs to the compiler surface, not to any vendor library.
IronPLC does not add vendor names to its compiler tables
([ADR-0042](../../adrs/0042-library-functions-over-compiler-intrinsics.md)
rule 1), so per the
[implementation plan](../../plans/2026-08-08-compatibility-library-bindings.md)
it ships as an ST function in the bundled **`Tc2_System`** library — the
library real TwinCAT projects reference by default — so it is in scope on
the paved path (compiling a real `.plcproj` project) without widening the
out-of-the-box compiler surface.

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
