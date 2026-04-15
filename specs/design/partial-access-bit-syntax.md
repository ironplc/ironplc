# Partial Access Bit Syntax (`.%Xn`)

**Implementation plan:** [`specs/plans/2026-04-15-partial-access-bit-syntax.md`](../plans/2026-04-15-partial-access-bit-syntax.md)

## Overview

IronPLC supports bit-level access on integer-typed variables using the vendor
short form `x.n` (for example, `byte_var.3`). This design adds support for the
IEC 61131-3:2013 standard form `x.%Xn` (for example, `byte_var.%X3`), which is
semantically equivalent. The new form is accepted on any symbolic variable,
including array elements and structured fields, producing the same AST and
bytecode as the existing short form.

The motivating case is a rusty program the compiler rejects with P0003:

```
myByteArray : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000];
r := myByteArray[0].%X0;                        (* TRUE *)
myByteArray[0].%X1 := TRUE;                     (* write *)
```

The syntax is gated behind `--allow-partial-access-syntax`, which is enabled
by the `rusty` and `iec61131-3-ed3` dialect presets.

This design covers only bit partial access (`.%Xn`). Byte/word/dword/lword
partial access (`.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`) returns a non-bit view of the
underlying data and requires a distinct AST node and codegen path; those are
out of scope here and will be covered by a separate design that can reuse the
same gating flag.

## Lexical Grammar

**REQ-PAB-001** The lexer recognizes a token `PartialAccessBit` matching the
regex `%X\d+`, case-insensitive (both `%X0` and `%x0` are accepted).

**REQ-PAB-002** The `PartialAccessBit` regex does not conflict with
`DirectAddress` (`%[IQM]([XBWDL])?(\d(\.\d)*)`) or `DirectAddressIncomplete`
(`%[IQM]\*`). A source containing `%IX0.0` still tokenizes as a single
`DirectAddress`, not as `PartialAccessBit`.

## Syntactic Grammar

**REQ-PAB-010** The `symbolic_variable` grammar rule accepts `.%Xn`
immediately following a symbolic variable reference, in the same positions
where `.n` is accepted.

**REQ-PAB-011** `.%Xn` is accepted after an array subscript, so
`arr[i].%Xn` parses successfully (the user's reported case
`myByteArray[0].%X0`).

**REQ-PAB-012** `.%Xn` is accepted after a structured field access, so
`record.field.%Xn` parses successfully.

## AST Representation

**REQ-PAB-020** `x.%Xn` and `x.n` produce equal AST trees under `PartialEq`
on `SymbolicVariableKind`: both lower to `BitAccessVariable { variable, index }`
with the same `index` value. No new AST variant is introduced.

## Semantic Analysis

**REQ-PAB-030** The existing `rule_bit_access_range` analyzer rule applies to
`.%Xn` identically to `.n`. A bit index outside the base type's bit width
produces the same `BitAccessOutOfRange` (`P4025`) diagnostic. For example,
`b.%X8` on a `BYTE` is rejected.

## Execution Semantics

**REQ-PAB-040** Reading `x.%Xn` on a BYTE / WORD / DWORD / LWORD variable
returns the value of bit n (1 if set, 0 if clear).

**REQ-PAB-041** Reading `arr[i].%Xn` on an array of integer-typed elements
returns the value of bit n of element i. Given
`arr : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000];`, the expression
`arr[0].%X0` evaluates to TRUE, `arr[0].%X1` evaluates to FALSE, and
`arr[0].%X2` evaluates to TRUE.

**REQ-PAB-042** Assigning `arr[i].%Xn := TRUE` sets bit n of element i;
assigning `arr[i].%Xn := FALSE` clears bit n. Other bits of the same element
and other elements are unchanged.

## Feature Gating

**REQ-PAB-050** When `allow_partial_access_syntax` is false (the Edition 2
default), a source containing `.%Xn` produces a `PartialAccessSyntaxDisabled`
(`P4033`) diagnostic pointing at the `%Xn` token — not the legacy `P0003
"Unmatched character sequence"`.

**REQ-PAB-051** `CompilerOptions::from_dialect(Dialect::Rusty)` sets
`allow_partial_access_syntax` to true.

**REQ-PAB-052** `CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)` sets
`allow_partial_access_syntax` to true.

## Round-Trip Rendering

**REQ-PAB-060** The plc2plc renderer emits `.n` (the short form) for every
`BitAccessVariable`, regardless of whether the source used `.%Xn` or `.n`.
Source parsed, rendered, and re-parsed produces an AST equal to the original.
The normalization is intentional: `BitAccessVariable` carries only an integer
index, so the surface syntax is not preserved in the AST.

## Requirements → Tests

Each REQ above is tied to one primary test. Test names follow
`{area}_spec_req_pab_{nnn}_{description}`.

| Requirement  | Test function                                                                   | File                                                              | Kind        |
|--------------|---------------------------------------------------------------------------------|-------------------------------------------------------------------|-------------|
| REQ-PAB-001  | `lexer_spec_req_pab_001_percent_x_digits_tokenizes_as_partial_access_bit`       | `compiler/parser/src/tests.rs`                                    | lexer       |
| REQ-PAB-002  | `lexer_spec_req_pab_002_direct_address_still_takes_precedence`                  | `compiler/parser/src/tests.rs`                                    | lexer       |
| REQ-PAB-010  | `parser_spec_req_pab_010_dot_percent_x_accepted_on_simple_var`                  | `compiler/parser/src/tests.rs`                                    | parser      |
| REQ-PAB-011  | `parser_spec_req_pab_011_dot_percent_x_accepted_after_array_subscript`          | `compiler/parser/src/tests.rs`                                    | parser      |
| REQ-PAB-012  | `parser_spec_req_pab_012_dot_percent_x_accepted_after_struct_field`             | `compiler/parser/src/tests.rs`                                    | parser      |
| REQ-PAB-020  | `parser_spec_req_pab_020_dot_percent_x_and_dot_n_produce_equal_ast`             | `compiler/parser/src/tests.rs`                                    | AST         |
| REQ-PAB-030  | `analyzer_spec_req_pab_030_dot_percent_x_bit_out_of_range_is_rejected`          | `compiler/analyzer/src/rule_bit_access_range.rs` (tests mod)      | analyzer    |
| REQ-PAB-040  | `codegen_spec_req_pab_040_read_percent_x_on_byte_returns_bit`                   | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-041  | `codegen_spec_req_pab_041_read_percent_x_on_byte_array_element_returns_bit`    | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e (user's case) |
| REQ-PAB-042  | `codegen_spec_req_pab_042_write_percent_x_on_byte_array_preserves_other_bits`  | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-050  | `parser_spec_req_pab_050_disabled_flag_produces_partial_access_syntax_disabled` | `compiler/parser/src/tests.rs`                                    | negative    |
| REQ-PAB-051  | `options_spec_req_pab_051_rusty_dialect_enables_partial_access_syntax`          | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-052  | `options_spec_req_pab_052_ed3_dialect_enables_partial_access_syntax`            | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-060  | `plc2plc_spec_req_pab_060_percent_x_round_trips_through_short_form`             | `compiler/plc2plc/src/tests.rs`                                   | round-trip  |

### Enforcement

The `#[spec_test]` proc-macro machinery in `compiler/container/build.rs` is
scoped to the container crate (bytecode format and instruction set); it does
not cover cross-crate language-feature requirements. This design therefore
uses the lighter naming-convention approach for traceability. Reviewers should
verify this table on every change.
