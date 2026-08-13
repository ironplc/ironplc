# Plan: `__TRUNC` / `__MOD` Compiler Intrinsics

## Goal

Expose the LREAL-preserving truncation and floating-modulo VM builtins
(`TRUNC_F64`/`MOD_F64`, merged in
[#1345](https://github.com/ironplc/ironplc/pull/1345)) as two callable
**compiler intrinsics** in the reserved `__` namespace:

- `__TRUNC(IN: ANY_REAL): ANY_REAL` — truncation toward zero; the result
  stays in the input's real type, so values beyond any integer type's range
  do not clamp.
- `__MOD(IN1: ANY_REAL, IN2: ANY_REAL): ANY_REAL` — IEEE-754 floating
  remainder with the sign of the dividend (fmod); `__MOD(x, 0.0)` is NaN,
  never a trap.

`ANY_REAL` genericity requires the REAL (f32) builtin variants, which this
plan adds (`TRUNC_F32`, `MOD_F32`).

## Why intrinsics instead of manifest bindings

The compatibility-library bindings design originally mapped library POUs to
these builtins through a manifest table
(`LTRUNC = { intrinsic = "..." }`). Review rejected that shape on security
grounds: it makes an on-disk data file an input to code *emission*, and
nothing structurally guarantees the library's declared signature matches the
builtin's stack behavior — a mismatched binding would corrupt the operand
stack. With `__TRUNC`/`__MOD` as ordinary typed intrinsics:

- Every `BUILTIN` emission originates from compiler-owned tables; manifests
  return to being pure metadata.
- The signature is type-checked by the analyzer like any stdlib function —
  the mismatch class cannot exist.
- Library functions like `Tc2_Math`'s `LTRUNC` become plain ST bodies
  (`LTRUNC := __TRUNC(IN);`), needing no mechanism at all.

The `__` prefix is the established compiler-provided namespace
(`__SYSTEM_UP_TIME`, `__ISVALIDREF`): names only the compiler can provide,
visibly non-portable, colliding with no IEC or vendor name. The intrinsics
are seeded unconditionally: their intended callers are bundled library
bodies, which are analyzed under the *user's* options, so gating them behind
a flag would break libraries for every user who did not pass it.

## Design

| Piece | Change |
|---|---|
| `container/src/opcode.rs` | `TRUNC_F32 = 0x03A5`, `MOD_F32 = 0x03A6`; `arg_count` arms |
| `vm/src/builtin.rs` | dispatch arms (`f32::trunc`, `%` on f32) |
| `project/src/disassemble.rs` | operand-name arms |
| `analyzer/.../stdlib_function.rs` | `get_compiler_intrinsic_functions()` seeding `__TRUNC`/`__MOD`, aggregated unconditionally |
| `codegen/src/compile_call.rs` | `lookup_builtin` arms via `float_builtin!` (width selects F32/F64) |
| `codegen/tests/it/wire_format.rs` | pin the two new func_ids |

## Non-goals

- No manifest `intrinsic` binding form (superseded; the bindings PR is
  reduced to declare-only).
- No vendor names (`LTRUNC` etc.) — those arrive as pure-ST library
  functions in a follow-up PR.
- No integer variants: `__TRUNC`/`__MOD` are `ANY_REAL`; the integer cases
  are already served by `TRUNC` and `MOD`.

## Testing strategy

- Analyzer: the signatures exist with `ANY_REAL` parameter/return types.
- VM: f32 truncation direction, fmod sign-of-dividend, fmod-by-zero → NaN
  (mirrors the existing f64 tests).
- End-to-end (parse → analyze → codegen → VM): `__TRUNC`/`__MOD` for both
  REAL and LREAL operands, negative values, epsilon asserts; proves the
  lexer accepts the `__` call spelling and width dispatch picks the right
  func_id.
- Wire-format pins for 0x03A5/0x03A6.
- `cd compiler && just` green.

## Tasks

- [ ] Plan (this document)
- [ ] F32 builtins: container + VM + disassembler + wire pins + VM tests
- [ ] Analyzer seeding + codegen lowering + e2e tests
- [ ] Full CI green; PR
