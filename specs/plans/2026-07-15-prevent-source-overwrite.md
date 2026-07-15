# Prevent Compiler Source Overwrite

## Goal

Reject a compile output path that refers to any loaded source file so the
compiler cannot replace source text with a bytecode container.

## Approach

Compare the existing output file with the sources enumerated by the project at
the final write boundary. Use file identity rather than path strings so relative
paths, case differences, symbolic links, and hard links are covered without
changing how unrelated existing output files are overwritten.

## File Map

- `compiler/ironplc-cli/src/cli.rs`: validate output file identity before writing.
- `compiler/ironplc-cli/tests/cli.rs`: prove rejection preserves source bytes.
- `compiler/ironplc-cli/Cargo.toml`: add the file-identity helper dependency.
- `compiler/problems/resources/problem-codes.csv`: add the filesystem diagnostic.
- `docs/reference/compiler/problems/P6009.rst`: document the diagnostic.

## Tasks

- [ ] Add the output/input conflict diagnostic and documentation.
- [ ] Reject output files that refer to loaded source files.
- [ ] Add exact-path and hard-link regression tests.
- [ ] Run the compiler and documentation checks.
