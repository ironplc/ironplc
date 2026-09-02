# Fix LEN rejecting a literal argument

Issue: [#1591](https://github.com/ironplc/ironplc/issues/1591)

## Goal

`LEN('Hello')` — the example published in `docs/reference/standard-library/functions/len.rst` — fails to compile with `P9999`. Make `LEN` accept the same argument shapes as every other string function (literal, variable, nested call), for both `STRING` and `WSTRING`.

## Architecture

`compile_len` matches only `ExprKind::Variable` and falls through to `Diagnostic::todo_with_span` for anything else. It never calls `resolve_string_arg`, the helper that gives the other string functions their literal and nested-call support, so the nested-call fix that added the catch-all arm to that helper never reached `LEN`.

Routing `compile_len` through `resolve_string_arg` fixes the narrow case on its own. The wide case needs more: `resolve_string_arg` allocates every intermediate slot at the narrow width, so a `WSTRING` literal is encoded as Latin-1 and a wide nested call (`LEN(MID(ws, 3, 1))`) traps at runtime with an encoding mismatch — the VM refuses a store whose source and destination headers disagree (ADR-0034). The width is statically known in every case: a literal spells it, a declaration states it, and each string function returns the encoding of its first string argument. Determining it up front lets one allocation path serve all three arms.

## Prefactoring

`resolve_string_arg` repeats the same slot-allocation block three times (bump `data_region_offset`, track `max_string_capacity`, emit `STR_INIT`), once per match arm. Making the width vary would mean editing that block in three places. Collapse it into one `allocate_string_temp` helper first, behaviour unchanged, so the width becomes a single parameter.

## File map

- `compiler/codegen/src/compile_string.rs` — modified
- `compiler/codegen/tests/it/end_to_end_len.rs` — modified
- `compiler/codegen/tests/it/end_to_end_wstring.rs` — modified

## Tasks

- [ ] Prefactor: extract `allocate_string_temp` from the three duplicated arms of `resolve_string_arg`
- [ ] Infer the encoding of a string-valued expression (literal, variable, array element, structure field, standard string function, user function return)
- [ ] Allocate the intermediate slot at the inferred width, and mark the program as holding a wide string so temp buffers are sized in wide bytes even when no `WSTRING` variable is declared
- [ ] Route `compile_len` through `resolve_string_arg`
- [ ] Test `LEN` of a literal, an empty literal, a nested call and a nested `CONCAT`, for `STRING` and `WSTRING`
- [ ] Test that a `WSTRING` literal in a string function argument is genuinely UTF-16LE
