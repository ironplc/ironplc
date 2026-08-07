# Plan: End-to-End Test for a TwinCAT Solution That References a Compatibility Library

## Goal

Validate, end to end through the `ironplcc` binary, that a realistic TwinCAT
solution — the Visual-Studio-style `.sln` → `.tsproj` → `.plcproj` nesting that
TcXaeShell produces — compiles when the `.plcproj` references the bundled
`Tc2_System` compatibility library. This is the "end to end tests and
validation with `.plcproj` files" follow-up called out on
[issue #1199](https://github.com/ironplc/ironplc/issues/1199): a contributor
provided a minimal real TwinCAT project (solution nesting, a function using
`PI`, a `Tc2_System` reference) and the library mechanism merged in
[#1318](https://github.com/ironplc/ironplc/pull/1318) /
[#1320](https://github.com/ironplc/ironplc/pull/1320) should make an
equivalent project check clean with no flags beyond a dialect.

The test project is *similar to* (not copied from) the contributor's project:
the same solution structure and the same `PI`-via-`Tc2_System` shape, with
independently authored logic. The point under test is the library mechanism,
not the specific domain logic.

## Non-goals

- Reading `.sln` or `.tsproj` content. Discovery finds `.plcproj` files by
  walking the directory tree; the solution files are present in the fixture to
  prove they do not confuse discovery, not to be parsed.
- Executing the project (codegen/VM). `check` — parse plus full semantic
  analysis — is the use case from the issue (linting TwinCAT projects in CI).
- New library content or new syntax support.

## Design doc reference

[compatibility-libraries.md](../design/compatibility-libraries.md):

- `REQ-CL-sources-001` — referenced libraries are read from the `.plcproj`.
- `REQ-CL-sources-003` — resolution by strict name match (`Tc2_System`).
- `REQ-CL-analyzer-001` — dormant by default: without the reference, `PI` must
  *not* resolve (the negative control proves the pass comes from activation).
- `REQ-CL-analyzer-003` — with the library active, `PI` resolves in constant
  expressions.

These markers already have `#[spec_test]`-style unit coverage in
`sources`/`project`; this plan adds binary-level coverage of the same chain,
which no existing test exercises (existing CLI tests stop at flat directories
with a bare `.plcproj`).

## Approach

### 1. Checked-in fixture: a realistic TwinCAT solution

`compiler/ironplc-cli/resources/test/twincat_tc2_system_solution/`:

```
TurntableSolution.sln                     (minimal VS solution file)
TurntableSolution/
  TurntableSolution.tsproj                (minimal TwinCAT system project)
  PlcTurntable/
    PlcTurntable.plcproj                  (Compile entries with Windows-style
                                           paths + PlaceholderReference to
                                           Tc2_System, the version-flexible
                                           form real templates emit)
    POUs/
      F_DegreesToRadians.TcPOU            (FUNCTION using PI in its body)
      MAIN.TcPOU                          (PROGRAM calling the function)
```

The function mirrors the load-bearing shape of the issue's real project — a
`VAR CONSTANT` initializer `d2r : LREAL := PI/180.0;` — with independently
authored logic. This exercises `PI` resolving *and folding* in a constant
expression, the exact worked example the design doc uses.

The tests pass `--dialect codesys`, the invocation from the issue, not
`--dialect twincat`: the `twincat` dialect does not (yet) enable
`allow_constant_initializer_expressions`, so the real-world initializer form
fails with P4037 under it before the library mechanism is reached. Verified
against the actual `PiExample_sln` project from the issue: it checks clean
under `--dialect codesys` and fails under `--dialect twincat` with P4037
followed by a P9998 internal error from
`rule_var_decl_const_initialized.rs` (the rule assumes
`xform_fold_initializer_expressions` always normalizes `SimpleExpr`
initializers, but the transform leaves them unfolded when it rejects with
P4037). Both the dialect gap and the P9998 are pre-existing findings tracked
separately, not addressed by this plan.

### 2. Tests in `compiler/ironplc-cli/tests/cli.rs`

Spawning the real binary is the part with no existing coverage: it exercises
argument parsing, directory canonicalization, recursive discovery past `.sln`/
`.tsproj`, `.plcproj` library-reference extraction, bundled-registry resolution
(including the development-layout fallback for locating `resources/libs`),
injection ahead of user source, and full analysis.

1. `check_when_twincat_solution_references_tc2_system_then_ok` —
   `ironplcc check --dialect twincat <fixture root>` succeeds with empty
   stdout. No `--library` flag: activation must come from the `.plcproj`
   reference alone.
2. `check_when_twincat_solution_library_reference_removed_then_pi_undefined` —
   copy the fixture to a temp directory, remove the `<PlaceholderReference>`
   element from the `.plcproj`, and expect failure with `P4007`
   (VariableUndefined) for `PI`. This is the negative control: same sources,
   same dialect, only the library reference differs — so the passing test above
   passes *because of* the library mechanism, not because `PI` is a builtin.

## Risks

- The fixture becomes the de-facto example of a supported TwinCAT layout; if
  discovery's layout rules change (e.g. reading `.tsproj` for real), the
  fixture should evolve with them rather than being deleted.
- String-surgery removal of the `<PlaceholderReference>` element in the
  negative test is brittle if the fixture's `.plcproj` is reformatted; the
  test fails loudly (element not found) rather than silently passing.
