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

**REQ-PAB-parser-001** The lexer recognizes a token `PartialAccessBit` matching the
regex `%X\d+`, case-insensitive (both `%X0` and `%x0` are accepted).

**REQ-PAB-parser-002** The `PartialAccessBit` regex does not conflict with
`DirectAddress` (`%[IQM]([XBWDL])?(\d(\.\d)*)`) or `DirectAddressIncomplete`
(`%[IQM]\*`). A source containing `%IX0.0` still tokenizes as a single
`DirectAddress`, not as `PartialAccessBit`.

## Syntactic Grammar

**REQ-PAB-parser-010** The `symbolic_variable` grammar rule accepts `.%Xn`
immediately following a symbolic variable reference, in the same positions
where `.n` is accepted.

**REQ-PAB-parser-011** `.%Xn` is accepted after an array subscript, so
`arr[i].%Xn` parses successfully (the user's reported case
`myByteArray[0].%X0`).

**REQ-PAB-parser-012** `.%Xn` is accepted after a structured field access, so
`record.field.%Xn` parses successfully.

## AST Representation

**REQ-PAB-parser-020** `x.%Xn` and `x.n` produce equal AST trees under `PartialEq`
on `SymbolicVariableKind`: both lower to `BitAccessVariable { variable, index }`
with the same `index` value. No new AST variant is introduced.

## Semantic Analysis

**REQ-PAB-analyzer-030** The existing `rule_bit_and_partial_access_range` analyzer rule applies to
`.%Xn` identically to `.n`. A bit index outside the base type's bit width
produces the same `BitAccessOutOfRange` (`P4025`) diagnostic. For example,
`b.%X8` on a `BYTE` is rejected.

## Execution Semantics

**REQ-PAB-codegen-040** Reading `x.%Xn` on a BYTE / WORD / DWORD / LWORD variable
returns the value of bit n (1 if set, 0 if clear).

**REQ-PAB-codegen-041** Reading `arr[i].%Xn` on an array of integer-typed elements
returns the value of bit n of element i. Given
`arr : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000];`, the expression
`arr[0].%X0` evaluates to TRUE, `arr[0].%X1` evaluates to FALSE, and
`arr[0].%X2` evaluates to TRUE.

**REQ-PAB-codegen-042** Assigning `arr[i].%Xn := TRUE` sets bit n of element i;
assigning `arr[i].%Xn := FALSE` clears bit n. Other bits of the same element
and other elements are unchanged.

## Feature Gating

**REQ-PAB-parser-050** When `allow_partial_access_syntax` is false (the Edition 2
default), a source containing `.%Xn` produces a `PartialAccessSyntaxDisabled`
(`P4033`) diagnostic pointing at the `%Xn` token — not the legacy `P0003
"Unmatched character sequence"`.

**REQ-PAB-parser-051** `CompilerOptions::from_dialect(Dialect::Rusty)` sets
`allow_partial_access_syntax` to true.

**REQ-PAB-parser-052** `CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)` sets
`allow_partial_access_syntax` to true.

## Round-Trip Rendering

**REQ-PAB-plc2plc-060** The plc2plc renderer emits `.n` (the short form) for every
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

**REQ-PAB-parser-100** The lexer recognizes the tokens `PartialAccessByte`,
`PartialAccessWord`, `PartialAccessDWord` and `PartialAccessLWord` matching
`%B\d+`, `%W\d+`, `%D\d+` and `%L\d+` respectively, case-insensitive. As
with `%X`, a direct address such as `%IB0` still tokenizes as `DirectAddress`.

**REQ-PAB-parser-101** `symbolic_variable` accepts `.%Bn`, `.%Wn`, `.%Dn` and `.%Ln`
in every position where `.%Xn` is accepted: after a simple variable, after an
array subscript (`arr[i].%B2`) and after a structured field (`s.value.%B1`).

### AST Representation

**REQ-PAB-parser-110** The wider forms lower to
`SymbolicVariableKind::PartialAccess(PartialAccessVariable { variable, size, index })`,
where `size: PartialAccessSize` is one of `Byte`, `Word`, `DWord` or `LWord`.
This is a distinct node from `BitAccessVariable`; `.%Xn` continues to lower
to `BitAccessVariable`.

### Semantic Analysis

**REQ-PAB-codegen-120** The type of a partial-access expression is the bit-string type
of the slice width: `BYTE`, `WORD`, `DWORD` or `LWORD` for `.%Bn`, `.%Wn`,
`.%Dn` and `.%Ln` respectively, independent of the base variable's type.

**REQ-PAB-analyzer-121** `rule_bit_and_partial_access_range` rejects a slice wider than
the base variable with `BitAccessOutOfRange` (`P4025`); `byte_var.%W0` is an
error because a `WORD` does not fit in a `BYTE`.

**REQ-PAB-analyzer-122** `rule_bit_and_partial_access_range` rejects an index past the
last slice with `BitAccessOutOfRange` (`P4025`). The valid range is
`0..(base_bytes / slice_bytes - 1)`: `word_var.%B1` is accepted and
`word_var.%B2` is rejected.

### Execution Semantics

**REQ-PAB-codegen-130** Reading `x.%Bn` / `x.%Wn` / `x.%Dn` on a wider base returns
the bits of slice `n`, shifted down so that the slice's least significant bit
is bit 0 of the result. Given `d : DWORD := 16#AABBCCDD`, `d.%B0` is `16#DD`,
`d.%B3` is `16#AA`, `d.%W1` is `16#AABB`; given
`l : LWORD := 16#AABBCCDD11223344`, `l.%D1` is `16#AABBCCDD`.

**REQ-PAB-codegen-131** Assigning `x.%Bn := v`, `x.%Wn := v`, `x.%Dn := v` or
`x.%Ln := v` replaces only the bits of slice `n` with the low bits of `v`;
every other bit of `x` is unchanged. Given `d : DWORD := 16#AABBCCDD`, after
`d.%B1 := 16#FF` the value of `d` is `16#AABBFFDD`. A slice as wide as its
base (`d.%D0` on a `DWORD`, `l.%L0` on an `LWORD`) replaces the whole value.

**REQ-PAB-codegen-133** The value assigned to a slice is compiled at the slice's own
width, unsigned: a 64-bit slice takes a 64-bit right-hand side (an `LWORD`
variable keeps all 64 bits, an `LWORD` literal is not range-checked as a
32-bit constant), and a 32-bit slice accepts a literal with its top bit set
(`DWORD#16#FFFFFFFF`) as a bit pattern.

**REQ-PAB-codegen-132** Reads and writes of a slice work on array elements
(`arr[0].%B2`) and structured fields (`s.value.%B2`) with the same semantics as
on a plain variable.

### Feature Gating

**REQ-PAB-parser-140** When `allow_partial_access_syntax` is false, a source containing
`.%Bn`, `.%Wn`, `.%Dn` or `.%Ln` produces `PartialAccessSyntaxDisabled`
(`P4033`) pointing at the selector token, the same diagnostic as `.%Xn`.

**REQ-PAB-parser-141** `CompilerOptions::from_dialect` sets
`allow_partial_access_syntax` to true for `Dialect::Codesys` and
`Dialect::TwinCat`, in addition to `Dialect::Rusty` and
`Dialect::Iec61131_3Ed3` (REQ-PAB-parser-051, REQ-PAB-parser-052).

### Round-Trip Rendering

**REQ-PAB-plc2plc-150** The plc2plc renderer emits `.%Bn`, `.%Wn`, `.%Dn` and `.%Ln`
verbatim for a `PartialAccessVariable`. Unlike `.%Xn`, these forms have no
short-form equivalent, so the rendered output still requires the flag to
re-parse.

## Requirements → Tests

Each requirement ID carries the slug of the crate that owns its conformance
test (see [cross-crate-spec-conformance.md](./cross-crate-spec-conformance.md)).
The `parser`, `analyzer`, `codegen` and `plc2plc` crates each list this
document in their `build.rs`, and every test below is annotated
`#[spec_test(REQ_PAB_<crate>_NNN)]`, so a requirement removed from this
document fails to compile its test and a requirement without a test fails
that crate's `all_spec_requirements_have_tests` meta-test. Test names follow
`{area}_spec_req_pab_{nnn}_{description}` where the test is dedicated to one
requirement; a parametrized table names the cases that cover each.

| Requirement  | Test function                                                                   | File                                                              | Kind        |
|--------------|---------------------------------------------------------------------------------|-------------------------------------------------------------------|-------------|
| REQ-PAB-parser-001  | `lexer_spec_req_pab_001_percent_x_digits_tokenizes_as_partial_access_bit`       | `compiler/parser/src/tests/`                                    | lexer       |
| REQ-PAB-parser-002  | `lexer_spec_req_pab_002_direct_address_still_takes_precedence`                  | `compiler/parser/src/tests/`                                    | lexer       |
| REQ-PAB-parser-010  | `parser_spec_req_pab_010_dot_percent_x_accepted_on_simple_var`                  | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-parser-011  | `parser_spec_req_pab_011_dot_percent_x_accepted_after_array_subscript`          | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-parser-012  | `parser_spec_req_pab_012_dot_percent_x_accepted_after_struct_field`             | `compiler/parser/src/tests/`                                    | parser      |
| REQ-PAB-parser-020  | `parser_spec_req_pab_020_dot_percent_x_and_dot_n_produce_equal_ast`             | `compiler/parser/src/tests/`                                    | AST         |
| REQ-PAB-analyzer-030  | `analyzer_spec_req_pab_030_dot_percent_x_bit_out_of_range_is_rejected`          | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod)      | analyzer    |
| REQ-PAB-codegen-040  | `codegen_spec_req_pab_040_read_percent_x_on_byte_returns_bit`                   | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-codegen-041  | `codegen_spec_req_pab_041_read_percent_x_on_byte_array_element_returns_bit`    | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e (user's case) |
| REQ-PAB-codegen-042  | `codegen_spec_req_pab_042_write_percent_x_on_byte_array_preserves_other_bits`  | `compiler/codegen/tests/end_to_end_bit_access.rs`                 | e2e         |
| REQ-PAB-parser-050  | `parser_spec_req_pab_050_disabled_flag_produces_partial_access_syntax_disabled` | `compiler/parser/src/tests/`                                    | negative    |
| REQ-PAB-parser-051  | `options_spec_req_pab_051_rusty_dialect_enables_partial_access_syntax`          | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-parser-052  | `options_spec_req_pab_052_ed3_dialect_enables_partial_access_syntax`            | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-plc2plc-060  | `plc2plc_spec_req_pab_060_percent_x_round_trips_through_short_form`             | `compiler/plc2plc/src/tests/`                                   | round-trip  |
| REQ-PAB-parser-100  | `lexer_spec_req_pab_100_percent_b_w_d_l_digits_tokenize_as_partial_access_selectors` | `compiler/parser/src/tests/partial_access.rs`                | lexer       |
| REQ-PAB-parser-101  | `parser_spec_req_pab_101_wider_selectors_accepted_in_every_position`         | `compiler/parser/src/tests/partial_access.rs`                     | parser      |
| REQ-PAB-parser-110  | `parser_spec_req_pab_110_wider_selector_lowers_to_partial_access_variable`    | `compiler/parser/src/tests/partial_access.rs`                     | AST         |
| REQ-PAB-codegen-120  | `partial_access_when_narrow_result_then_expected` (`word_1_of_dword`, result assigned to a `WORD`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-analyzer-121  | `apply_when_partial_access_wider_than_variable_then_err`                        | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod) | analyzer |
| REQ-PAB-analyzer-122  | `apply_when_partial_access_index_at_boundary_then_ok_or_err`                    | `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` (tests mod) | analyzer |
| REQ-PAB-codegen-130  | `partial_access_when_narrow_result_then_expected` (`byte_0_of_dword`, `byte_3_of_dword`, `word_1_of_dword`, `dword_1_of_lword`), `partial_access_when_wide_result_then_expected` (`lword_0_of_lword`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-codegen-131  | `partial_access_when_narrow_result_then_expected` (`write_byte_1_of_dword`, `write_dword_0_of_dword`), `partial_access_when_wide_result_then_expected` (`write_word_1_of_lword`, `write_dword_1_of_lword`, `write_lword_0_of_lword`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-codegen-132  | `partial_access_when_narrow_result_then_expected` (`write_byte_3_of_array_element`, `write_byte_2_of_struct_field`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-codegen-133  | `partial_access_when_wide_result_then_expected` (`write_dword_0_of_lword`, `write_lword_0_of_lword`, `write_lword_0_of_lword_from_variable`) | `compiler/codegen/tests/it/end_to_end_partial_access.rs` | e2e |
| REQ-PAB-parser-140  | `apply_when_partial_access_byte_and_flag_off_then_error`                        | `compiler/parser/src/rule_token_no_partial_access_syntax.rs` (tests mod) | negative |
| REQ-PAB-parser-141  | `options_spec_req_pab_141_vendor_dialects_enable_partial_access_syntax`       | `compiler/parser/src/options.rs` (tests mod)                      | options     |
| REQ-PAB-plc2plc-150  | `plc2plc_when_partial_access_multi_then_round_trips`                            | `compiler/plc2plc/src/tests/partial_access.rs`                    | round-trip  |

### Enforcement

`compiler/parser/build.rs`, `compiler/analyzer/build.rs`,
`compiler/codegen/build.rs` and `compiler/plc2plc/build.rs` list this
document, so each crate's build generates a constant per requirement it owns
and its `all_spec_requirements_have_tests` meta-test fails if one has no
`#[spec_test]`. The workspace guard in `compiler/test` fails if a requirement
names a crate that does not list this document.
