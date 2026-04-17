# Implementation Plan: Complete Bit Access Support

**Builds on:** PR #916 (partial-access bit syntax), PR #918 (deferred bit-access paths)

## Goal

Fill in the two remaining unsupported bit/partial-access use cases:

1. **Bit access on arrays nested in struct fields** (`s.my_array[0].0`) —
   codegen-only fix; parser and analyzer already handle this correctly.
2. **`.%Bn`/`.%Wn`/`.%Dn`/`.%Ln` byte/word/dword/lword partial access** —
   full pipeline: lexer, parser, AST, analyzer, codegen, renderer.

## Feature 1: Struct-Field Array Bit Access

### Problem

The codegen bit-access paths only know about top-level arrays
(`ctx.array_vars`) and top-level struct fields (`ctx.struct_vars`). When
the base of a bit access is an array inside a struct (AST shape:
`BitAccess { variable: Array { subscripted_variable: Structured {...} } }`),
two things go wrong:

- **Read**: `base_op_type` defaults to `DEFAULT_OP_TYPE` instead of deriving
  the element width from the struct field's array element type. This causes
  wrong shift width for 64-bit elements.
- **Write**: `compile_bit_access_assignment_on_array` fails at
  `ctx.array_vars.get(root_name)` because the root is a struct, not an array.

### Approach

Detect the `Array(Structured(...))` pattern and reuse the existing
`resolve_struct_field_array()` from `compile_array.rs`, which already handles
non-bit struct-field array reads/writes via `StructFieldArrayElement`.

### File Map

| File | Change |
|------|--------|
| `compiler/codegen/src/compile_expr.rs` | Fix `base_op_type` for struct-field array in read path; add `compile_bit_access_assignment_on_struct_field_array()` for write path |
| `compiler/codegen/tests/end_to_end_bit_access_not_impl.rs` | Add e2e tests |

## Feature 2: Multi-Byte Partial Access

### Semantics

| Syntax | Width | Result Type | Example |
|--------|-------|-------------|---------|
| `.%Bn` | 8-bit | BYTE | `DWORD_VAR.%B2` → bits 16-23 |
| `.%Wn` | 16-bit | WORD | `LWORD_VAR.%W1` → bits 16-31 |
| `.%Dn` | 32-bit | DWORD | `LWORD_VAR.%D1` → bits 32-63 |
| `.%Ln` | 64-bit | LWORD | `LWORD_VAR.%L0` → all 64 bits |

### File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/textual.rs` | New `PartialAccessSize` enum, `PartialAccessVariable` struct, `SymbolicVariableKind::PartialAccess` variant |
| `compiler/dsl/src/fold.rs` | Add `dispatch!(PartialAccessVariable)` |
| `compiler/parser/src/token.rs` | New tokens: `PartialAccessByte`, `PartialAccessWord`, `PartialAccessDWord`, `PartialAccessLWord` |
| `compiler/parser/src/parser.rs` | Extend `symbolic_variable()` grammar with new token alternatives |
| `compiler/parser/src/rule_token_no_partial_access_syntax.rs` | Extend gating to reject new tokens when flag is off |
| `compiler/analyzer/src/rule_bit_access_range.rs` | Add range validation for partial access |
| `compiler/analyzer/src/xform_resolve_late_bound_expr_kind.rs` | Handle new variant |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Type inference for partial access |
| `compiler/codegen/src/compile_expr.rs` | Read/write codegen for partial access |
| `compiler/codegen/src/compile_stmt.rs` | Assignment dispatch for partial access |
| `compiler/plc2plc/src/renderer.rs` | Render `.%Bn`/`.%Wn`/`.%Dn`/`.%Ln` |
| `compiler/plc2plc/src/tests.rs` | Round-trip test |
| `compiler/codegen/tests/end_to_end_partial_access.rs` | E2e execution tests |
| `compiler/parser/src/tests.rs` | Lexer/parser tests |

## Tasks

- [ ] Create failing e2e tests for struct-field array bit access
- [ ] Fix read path `base_op_type` for struct-field arrays
- [ ] Add write path for struct-field array bit access
- [ ] Add lexer tokens for `.%Bn`/`.%Wn`/`.%Dn`/`.%Ln`
- [ ] Extend gating rule for new tokens
- [ ] Add `PartialAccessVariable` AST types
- [ ] Extend parser for new tokens
- [ ] Add analyzer validation for partial access ranges
- [ ] Update analyzer match arms for new variant
- [ ] Add codegen for partial access read/write
- [ ] Update plc2plc renderer
- [ ] Add round-trip tests
- [ ] Add e2e execution tests
- [ ] Add parser/lexer tests
- [ ] `cd compiler && just` passes
