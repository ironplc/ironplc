# Plan: Expanded Implicit Type Widening

## Goal

Support additional type widening cases that work in RuSTy but not in IronPLC: integer→real (lossless), bit-string widening, and cross-family bit-string↔integer widening.

## Architecture

Standard-compliant widening (integer→real lossless, bit-string chain) is enabled by default. Cross-family widening (bit-string↔integer, literal→bit-string) is gated behind `--allow-cross-family-widening`, enabled in the Rusty dialect.

## Design doc reference

See ADR-0031: `specs/adrs/0031-expanded-implicit-type-widening.md`

## File map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | Replace `integer_properties()` with `type_properties()`, expand `can_widen_to()`, add `can_widen_cross_family_to()` |
| `compiler/analyzer/src/rule_function_call_type_check.rs` | Thread `CompilerOptions`, add cross-family logic, update/add tests |
| `compiler/parser/src/options.rs` | Add `allow_cross_family_widening` flag |
| `compiler/plc2x/bin/main.rs` | Wire CLI flag |
| `compiler/plc2x/src/lsp.rs` | Wire LSP flag |
| `docs/explanation/enabling-dialects-and-features.rst` | Document new flag |
| `docs/reference/compiler/ironplcc.rst` | Document new flag |

## Phases

1. ADR and plan (this file)
2. Standard widening: expand `can_widen_to()` for integer→real and bit-string chains
3. Cross-family widening: add flag, thread options, implement gated logic
4. Documentation updates
5. Full CI verification
