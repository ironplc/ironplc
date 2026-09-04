# Partial Access Syntax (`.%Xn`, `.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`)

## Overview

IronPLC supports bit-level access on integer-typed variables using the non-standard
short form `x.n` (for example, `byte_var.3`). This design adds support for the
IEC 61131-3:2013 standard form `x.%Xn` (for example, `byte_var.%X3`), which is
semantically equivalent. The new form is accepted on any symbolic variable,
including array elements and structured fields, producing the same AST and
bytecode as the existing short form.

The same syntax family also selects wider slices: `x.%Bn` (byte), `x.%Wn`
(word), `x.%Dn` (double word) and `x.%Ln` (long word). Those forms return a
bit-string view of the underlying data rather than a `BOOL`, so they have their
own AST node and codegen path; see [Multi-Byte Partial
Access](#multi-byte-partial-access) below.

The motivating case is a rusty program the compiler rejects with P0003:

```
myByteArray : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000];
r := myByteArray[0].%X0;                        (* TRUE *)
myByteArray[0].%X1 := TRUE;                     (* write *)
```

All five forms are gated behind the one flag `--allow-partial-access-syntax`,
which the `iec61131-3-ed3`, `rusty`, `codesys` and `twincat` dialect presets
enable.

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

**REQ-PAB-030** The existing `rule_bit_and_partial_access_range` analyzer rule applies to
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

## Multi-Byte Partial Access

The byte, word, dword and lword forms select a slice of the base value and
read and write it as the bit-string type of the slice's width. Slice `n`
covers bits `n * width` through `(n + 1) * width - 1`; index 0 is the least
significant slice.

| form   | width  | result type | example                              |
|--------|--------|-------------|--------------------------------------|
| `.%Bn` | 8-bit  | BYTE        | `DWORD_VAR.%B2` → bits 16-23         |
| `.%Wn` | 16-bit | WORD        | `LWORD_VAR.%W1` → bits 16-31         |
| `.%Dn` | 32-bit | DWORD       | `LWORD_VAR.%D1` → bits 32-63         |
| `.%Ln` | 64-bit | LWORD       | `LWORD_VAR.%L0` → all 64 bits        |

### Lexical and Syntactic Grammar

**REQ-PAB-100** The lexer recognizes the tokens `PartialAccessByte`,
`PartialAccessWord`, `PartialAccessDWord` and `PartialAccessLWord` matching
`%B\d+`, `%W\d+`, `%D\d+` and `%L\d+` respectively, case-insensitive. As
with `%X`, a direct address such as `%IB0` still tokenizes as `DirectAddress`.

**REQ-PAB-101** `symbolic_variable` accepts `.%Bn`, `.%Wn`, `.%Dn` and `.%Ln`
in every position where `.%Xn` is accepted: after a simple variable, after an
array subscript (`arr[i].%B2`) and after a structured field (`s.value.%B1`).

### AST Representation

**REQ-PAB-110** The wider forms lower to
`SymbolicVariableKind::PartialAccess(PartialAccessVariable { variable, size, index })`,
where `size: PartialAccessSize` is one of `Byte`, `Word`, `DWord` or `LWord`.
This is a distinct node from `BitAccessVariable`; `.%Xn` continues to lower
to `BitAccessVariable`.

### Semantic Analysis

**REQ-PAB-120** The type of a partial-access expression is the bit-string type
of the slice width: `BYTE`, `WORD`, `DWORD` or `LWORD` for `.%Bn`, `.%Wn`,
`.%Dn` and `.%Ln` respectively, independent of the base variable's type.

**REQ-PAB-121** `rule_bit_and_partial_access_range` rejects a slice wider than
the base variable with `BitAccessOutOfRange` (`P4025`); `byte_var.%W0` is an
error because a `WORD` does not fit in a `BYTE`.

**REQ-PAB-122** `rule_bit_and_partial_access_range` rejects an index past the
last slice with `BitAccessOutOfRange` (`P4025`). The valid range is
`0..(base_bytes / slice_bytes - 1)`: `word_var.%B1` is accepted and
`word_var.%B2` is rejected.

### Execution Semantics

**REQ-PAB-130** Reading `x.%Bn` / `x.%Wn` / `x.%Dn` on a wider base returns
the bits of slice `n`, shifted down so that the slice's least significant bit
is bit 0 of the result. Given `d : DWORD := 16#AABBCCDD`, `d.%B0` is `16#DD`,
`d.%B3` is `16#AA`, `d.%W1` is `16#AABB`; given
`l : LWORD := 16#AABBCCDD11223344`, `l.%D1` is `16#AABBCCDD`.

**REQ-PAB-131** Assigning `x.%Bn := v` / `x.%Wn := v` replaces only the bits of
slice `n` with the low bits of `v`; every other bit of `x` is unchanged. Given
`d : DWORD := 16#AABBCCDD`, after `d.%B1 := 16#FF` the value of `d` is
`16#AABBFFDD`.

**REQ-PAB-132** Reads and writes of a slice work on array elements
(`arr[0].%B2`) and structured fields (`s.value.%B2`) with the same semantics as
on a plain variable.

### Feature Gating

**REQ-PAB-140** When `allow_partial_access_syntax` is false, a source containing
`.%Bn`, `.%Wn`, `.%Dn` or `.%Ln` produces `PartialAccessSyntaxDisabled`
(`P4033`) pointing at the selector token, the same diagnostic as `.%Xn`.

**REQ-PAB-141** `CompilerOptions::from_dialect` sets
`allow_partial_access_syntax` to true for `Dialect::Codesys` and
`Dialect::TwinCat`, in addition to `Dialect::Rusty` and
`Dialect::Iec61131_3Ed3` (REQ-PAB-051, REQ-PAB-052).

### Round-Trip Rendering

**REQ-PAB-150** The plc2plc renderer emits `.%Bn`, `.%Wn`, `.%Dn` and `.%Ln`
verbatim for a `PartialAccessVariable`. Unlike `.%Xn`, these forms have no
short-form equivalent, so the rendered output still requires the flag to
re-parse.

## Requirements → Tests

Each REQ above is tied to one primary test. Test names follow
`{area}_spec_req_pab_{nnn}_{description}`.

| Requirement  | Test function                                                                   | File                                                              | Kind        |
|--------------|---------------------------------------------------------------------------------|-------------------------------------------------------------------|-------------|
| REQ-PAB-001  | `lexer_spec_req_pab_001_percent_x_digits_tokenizes_as_partial_access_bit`       | `compiler/parser/src/tests/`                                    | lexer       |
| REQ-PAB-002  | `lexer_spec_req_pab_002_direct_address_still_takes_precedence`                  | `compiler/parser/src/tests/`                                    | lexer       |
| REQ-PAB-010  | `parser_spec_req_pab_010_dot_percent_x_accepted_on_simple_var`                  | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-011  | `parser_spec_req_pab_011_dot_percent_x_accepted_after_array_subscript`          | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-012  | `parser_spec_req_pab_012_dot_percent_x_accepted_after_struct_field`             | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-020  | `parser_spec_req_pab_020_dot_percent_x_and_dot_n_produce_equal_ast`             | `compiler/parser/src/tests/`                                    | AST         |
| REQ-PAB-030  | `analyzer_spec_req_pab_030_dot_percent_x_bit_out_of_range_is_rejected`          | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod)      | analyzer    |
| REQ-PAB-040  | `codegen_spec_req_pab_040_read_percent_x_on_byte_returns_bit`                   | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-041  | `codegen_spec_req_pab_041_read_percent_x_on_byte_array_element_returns_bit`    | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e (user's case) |
| REQ-PAB-042  | `codegen_spec_req_pab_042_write_percent_x_on_byte_array_preserves_other_bits`  | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-050  | `parser_spec_req_pab_050_disabled_flag_produces_partial_access_syntax_disabled` | `compiler/parser/src/tests/`                                    | negative    |
| REQ-PAB-051  | `options_spec_req_pab_051_rusty_dialect_enables_partial_access_syntax`          | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-052  | `options_spec_req_pab_052_ed3_dialect_enables_partial_access_syntax`            | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-060  | `plc2plc_spec_req_pab_060_percent_x_round_trips_through_short_form`             | `compiler/plc2plc/src/tests/`                                   | round-trip  |
| REQ-PAB-100  | `lexer_spec_req_pab_100_percent_b_w_d_l_digits_tokenize_as_partial_access_selectors` | `compiler/parser/src/tests/partial_access.rs`                | lexer       |
| REQ-PAB-101  | `end_to_end_when_read_byte_from_dword_array_then_correct`, `end_to_end_when_read_byte_from_struct_field_then_correct` | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-110  | `end_to_end_when_partial_access_byte_flag_on_then_compiles`                     | `compiler/codegen/tests/it/end_to_end_partial_access.rs`          | e2e         |
| REQ-PAB-120  | `end_to_end_when_read_word_1_from_dword_then_correct` (result assigned to a `WORD`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs`      | e2e         |
| REQ-PAB-121  | `apply_when_partial_access_wider_than_variable_then_err`                        | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod) | analyzer |
| REQ-PAB-122  | `apply_when_partial_access_index_at_boundary_then_ok_or_err`                    | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod) | analyzer |
| REQ-PAB-130  | `end_to_end_when_read_byte_0_from_dword_then_correct`, `end_to_end_when_read_byte_3_from_dword_then_correct`, `end_to_end_when_read_word_1_from_dword_then_correct`, `end_to_end_when_read_dword_1_from_lword_then_correct` | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-131  | `end_to_end_when_write_byte_1_to_dword_then_preserves_others`, `end_to_end_when_write_word_to_lword_then_correct` | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-132  | `end_to_end_when_write_byte_to_dword_array_then_correct`, `end_to_end_when_write_byte_to_struct_field_then_correct` | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-140  | `apply_when_partial_access_byte_and_flag_off_then_error`                        | `compiler/parser/src/rule_token_no_partial_access_syntax.rs` (tests mod) | negative |
| REQ-PAB-141  | `codesys_dialect_enables_exactly_these_flags`, `twincat_dialect_enables_exactly_these_flags` | `compiler/parser/src/options.rs` (tests mod)               | options     |
| REQ-PAB-150  | `plc2plc_when_partial_access_multi_then_round_trips`                            | `compiler/plc2plc/src/tests/partial_access.rs`                    | round-trip  |

### Enforcement

The `#[spec_test]` proc-macro machinery in `compiler/container/build.rs` is
scoped to the container crate (bytecode format and instruction set); it does
not cover cross-crate language-feature requirements. This design therefore
uses the lighter naming-convention approach for traceability. Reviewers should
verify this table on every change.
