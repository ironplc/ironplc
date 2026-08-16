# Plan: Resolve expression types for function block member access

## Problem

Reading a function block output in an expression that needs the expression's own
resolved type — for example `IF timer.Q THEN` — fails codegen with `P9999`
pointing at `codegen/src/compile_expr.rs` (`op_type`). Reported in
[issue #1375](https://github.com/ironplc/ironplc/issues/1375).

Minimal reproduction:

```iecst
PROGRAM p
VAR
  tmr : TON;
  done : BOOL := FALSE;
END_VAR
  IF tmr.Q THEN      (* P9999 *)
    done := TRUE;
  END_IF;
END_PROGRAM
```

`done := tmr.Q;` compiles because the assignment path derives the op type from
the assignment target, never consulting `Expr::resolved_type`. A condition has
no such target, so `condition_op_type` falls through to `op_type`, which
requires `resolved_type` to be populated and reports "not implemented" when it
is not.

## Root cause

`xform_resolve_expr_types` annotates `Expr::resolved_type`. Its struct-chain
walk resolves member access only through `IntermediateType::Structure`:

- `TypeEnvironment::resolve_struct_type` matches `Structure` and nothing else.
- `resolve_structured_variable_type` / `resolve_parent_struct_type` /
  `resolve_struct_field_array_element_type` all destructure
  `IntermediateType::Structure { fields }`.

A function block instance is registered as
`IntermediateType::FunctionBlock { name, fields }` — it carries the same
`IntermediateStructField` list (`TON` has `IN`, `PT`, `Q`, `ET`), but no branch
matches it. So `tmr.Q` resolves to `None` and the annotation is left unset.

Codegen already handles the FB member *read* correctly
(`compile_variable_read` emits `FB_LOAD_INSTANCE` + `FB_LOAD_PARAM` for
`ctx.fb_instances`). Only the analyzer annotation is missing, so the fix belongs
in the analyzer.

## Design Decisions

- Fix in the analyzer, not codegen. Codegen's requirement that the analyzer
  populate `resolved_type` is correct; the analyzer was not holding up its end.
- Treat `Structure` and `FunctionBlock` uniformly for *member access only*. Both
  expose a named `IntermediateStructField` list, so a shared accessor removes
  the duplicated `match` arms rather than adding a parallel FB code path.
- Keep `resolve_struct_type` unchanged. Other callers use it to ask "is this
  specifically a structure?" and must keep that meaning; add a separate
  member-access-oriented lookup instead.

## Implementation Steps

### Step 1: Add a member-field accessor to `IntermediateType`

**File**: `compiler/analyzer/src/intermediate_type.rs`

Add `member_fields()` returning `Option<&Vec<IntermediateStructField>>` for
`Structure` and `FunctionBlock`, `None` otherwise, and `has_members()` for the
matching predicate.

### Step 2: Add a member-access type lookup to `TypeEnvironment`

**File**: `compiler/analyzer/src/type_environment.rs`

Add `resolve_member_access_type(&self, type_name)` returning the representation
when it is a `Structure` or a `FunctionBlock`.

### Step 3: Use the new accessors in the type-resolution transform

**File**: `compiler/analyzer/src/xform_resolve_expr_types.rs`

- `resolve_parent_struct_type`: `Named` arm uses `resolve_member_access_type`;
  `Structured` arm uses `member_fields()` and `has_members()`.
- `resolve_structured_variable_type`: use `member_fields()`.
- `resolve_struct_field_array_element_type`: use `member_fields()`.

### Step 4: Tests

- `compiler/analyzer/src/xform_resolve_expr_types.rs`: unit test that a
  `timer.Q` condition gets a resolved type.
- `compiler/codegen/tests/it/end_to_end_fb_ton.rs`: end-to-end cases driving a
  TON whose `Q` is read from an `IF` condition and from a boolean expression,
  covering both a BOOL output (`Q`) and a TIME output (`ET`) compared in a
  condition.

## Verification

- Original issue #1375 program compiles.
- `cd compiler && just` passes (compile, coverage ≥ 85%, clippy, fmt).
