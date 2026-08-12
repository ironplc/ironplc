# Implementation Plan: `Tc2_Math` Compatibility Library

## Goal

Implement the `Tc2_Math` compatibility library (`LTRUNC`, `LMOD`, `MODABS`,
`FRAC`) exactly as specified by the clean-room behavior specification at
[specs/design/library-interfaces/tc2-math.md](../design/library-interfaces/tc2-math.md),
which is the **sole input** to this implementation per the
[Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md).

## Design doc reference

- [specs/design/library-interfaces/tc2-math.md](../design/library-interfaces/tc2-math.md)
  — the behavior spec (signatures, normative IEEE-754 edge semantics, test
  vectors, package contents, acceptance criteria)
- [specs/design/compatibility-libraries.md](../design/compatibility-libraries.md)
  — the library mechanism (`REQ-CL-*`), including `REQ-CL-analyzer-004`
  (user declaration shadows an activated library declaration)

## Architecture

- **Library package as data.** A new bundled package
  `compiler/sources/resources/libs/Tc2_Math/` (manifest + one `.st` file),
  following the existing `Tc2_System`/`Tc2_BuiltIns` layout. Every function is
  a fully-defined ST body; the two semantics ST cannot express are reached
  through the `__TRUNC`/`__MOD` compiler intrinsics (merged in #1348).
  `MODABS` and `FRAC` are math-dictated compositions per the spec; both call
  the intrinsics directly so their behavior cannot be altered by a user
  shadowing `LMOD`/`LTRUNC`.
- **Reference-activated only.** No discovery-module implicit list entry — the
  existing activation channels (`.plcproj` reference resolution and
  `--library`) already cover `Tc2_Math` by name with zero new code.
- **Function shadowing.** Acceptance criterion 4 (a user-defined `LTRUNC`
  takes precedence over the library's) does not work today: the merged
  compilation unit would carry two `FUNCTION LTRUNC` declarations and the
  function environment rejects the duplicate (`FunctionDeclNameDuplicated`).
  Fix: a filter in `sources/src/libraries/mod.rs` that drops a library
  `FUNCTION` declaration when user source declares a function of the same
  (case-insensitive) name, applied where compat libraries are injected ahead
  of user source (`project/src/project.rs::run_semantic_analysis`). This
  implements `REQ-CL-analyzer-004` for functions.
- **End-to-end tests in the codegen integration suite.** The codegen test
  crate gains a dev-dependency on `ironplc-sources` so tests can activate the
  bundled `Tc2_Math` through the real registry (activate → analyze → codegen
  → VM run) and pin every spec test vector.

## File map

| File | Change |
|---|---|
| `compiler/sources/resources/libs/Tc2_Math/library.toml` | New — manifest (`name`, `vendor`, `default_version`, `references`; no bindings) |
| `compiler/sources/resources/libs/Tc2_Math/1.0.0/Tc2_Math.st` | New — the four ST function bodies |
| `compiler/sources/src/libraries/mod.rs` | Add shadowed-function filter + loader/filter unit tests |
| `compiler/project/src/project.rs` | Apply the filter in `run_semantic_analysis`; shadowing + activation tests |
| `compiler/codegen/Cargo.toml` | Add `ironplc-sources` dev-dependency |
| `compiler/codegen/tests/it/end_to_end_tc2_math.rs` | New — every spec test vector end-to-end, shadowing end-to-end |
| `compiler/codegen/tests/it/main.rs` | Register the new test module |
| `compiler/ironplc-cli/resources/test/twincat_tc2_math_solution/…` | New fixture — solution referencing `Tc2_Math` |
| `compiler/ironplc-cli/tests/cli.rs` | `.plcproj` activation test + dormant-by-default negative test |
| `compiler/plc2plc/src/tests/…` | Round-trip test for source calling the four functions |

## Tasks

- [x] Commit this plan
- [x] Author `library.toml` and `Tc2_Math.st` from the spec
- [x] Loader tests: bundled registry contains and loads `Tc2_Math` (criterion 1)
- [x] Shadowed-function filter in `sources`, applied in `project`; tests (criterion 4)
- [x] End-to-end vector tests (criterion 2): every vector, exact vs `1.0E-9`
      per the spec tables; NaN rows assert `is_nan`, never a trap; shadowing
      end-to-end
- [x] `.plcproj` activation fixture + positive/negative CLI tests (criterion 3)
- [x] `plc2plc` round-trip test (criterion 5)
- [x] Full CI green (`cd compiler && just`) (criterion 6)
