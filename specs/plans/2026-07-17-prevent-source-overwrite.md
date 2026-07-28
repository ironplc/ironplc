# Prevent Compiler Source Overwrite

## Goal

Reject a `compile` output path that refers to any loaded source file so the
compiler cannot replace source text with a bytecode container.

## Approach

Check the output path against the loaded sources immediately after the project
is created, before parsing, analysis, or code generation. Compare paths with
`std::fs::canonicalize` so relative-vs-absolute differences and symbolic links
resolve to the same target; a non-existent output path canonicalizes to an
error and is treated as no conflict. No new dependency is required. Pure
hard-link aliasing (two names, one inode) is out of scope.

Emit the new `P6009` problem code so the failure links to the online docs.

## File Map

- `compiler/problems/resources/problem-codes.csv`: add the `P6009` diagnostic.
- `compiler/ironplc-cli/src/cli.rs`: reject the output path before writing.
- `compiler/ironplc-cli/tests/cli.rs`: prove rejection preserves source bytes.
- `docs/reference/compiler/problems/P6009.rst`: document the diagnostic.
- `docs/reference/compiler/problems/index.rst`: list the new page.

## Tasks

- [x] Add the `P6009` diagnostic and documentation.
- [x] Reject output paths that refer to a loaded source file (canonicalized).
- [x] Add exact-path and relative-vs-absolute regression tests.
- [ ] Run the compiler and documentation checks.
