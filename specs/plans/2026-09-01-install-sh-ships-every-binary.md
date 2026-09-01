# install.sh ships every built binary

Implements the fix for
[#1564](https://github.com/ironplc/ironplc/issues/1564): `compiler/install.sh`
installs `ironplcc`, `ironplcvm` and `ironplcmcp` but not `ironplcvmd`, and
`shipped_binaries_guard.rs` never reads `install.sh`, so nothing caught it.

## Context

`compiler/justfile:8` is the list the Unix tarball is built from:

```
binaries := "ironplcc ironplcvm ironplcmcp ironplcvmd"
```

`install.sh` extracts that tarball into a temp directory and moves out only the
names in `REQUIRED_BINARIES` and `OPTIONAL_BINARIES`. `ironplcvmd` is in
neither, so the `trap … EXIT` deletes it with the temp directory: the debug
server is downloaded, checksum-verified, extracted, and then thrown away.

`curl … | sh` is the only documented Linux install path, and two reference
pages tell the user the opposite — `docs/reference/editor/debugging.rst` says
the debug server "installs alongside the compiler", and `E0007`, the very error
these users hit, says "official installers place both together".

`shipped_binaries_guard.rs` exists to prevent exactly this and its module doc
claims it does, but its sources are Cargo `[[bin]]`s, `compiler/justfile`,
`compiler/setup.nsi` and the Homebrew formula. `install.sh` is a fifth
installer it never reads.

## Decisions

**All four binaries are required.** The compiler, the runtime, the MCP server
and the debug server are one toolchain; the editor extension resolves
`ironplcvmd` from beside `ironplcc`. `install.sh` therefore keeps one list, not
a required tier and an optional tier.

**One documented back-compatibility window.** The latest published release,
v0.235.0, predates `ironplcvmd` — its tarball contains only `ironplcc`,
`ironplcvm` and `ironplcmcp` (verified by listing the published archive). CI
runs `just install-script-smoke` against the last published release on every
pull request, so a bare `die` on any absent binary turns that job red until the
next release ships. A single `LEGACY_OPTIONAL_BINARIES` list names the binaries
a published release may legitimately lack; absence of anything else is fatal.
The list is emptied once no supported release predates its entries.

## Prefactoring

`just_binaries()` in `shipped_binaries_guard.rs` parses
`binaries := "a b c"` — the first quoted string on a named line, split on
whitespace. `install.sh`'s `BINARIES="a b c"` has the same shape. Extract
`whitespace_list_assignment(text, key)` and have both callers use it, in its
own behaviour-preserving commit, before adding the install.sh source.

## Steps

1. **Prefactor** — extract `whitespace_list_assignment` from `just_binaries`;
   existing fixture tests pass unchanged.
2. **`compiler/install.sh`** — collapse `REQUIRED_BINARIES` /
   `OPTIONAL_BINARIES` into `BINARIES` (all four) plus
   `LEGACY_OPTIONAL_BINARIES` (`ironplcvmd`). A binary missing from the archive
   warns when legacy-optional and dies otherwise. `already_installed_same_version`
   skips legacy-optional names so an old release stays idempotent.
3. **`shipped_binaries_guard.rs`** — parse `install.sh` as a fifth source:
   `BINARIES` must equal the built set, and `LEGACY_OPTIONAL_BINARIES` must be a
   subset of `BINARIES`. Update the module doc from four sources to five. Add
   fixture tests for the new parser.
4. **`justfile`** — `_install-script-smoke-verify` drives a DAP `initialize`
   handshake against `ironplcvmd` when it is installed, the way it already does
   an MCP handshake against `ironplcmcp`. Nothing anywhere smoke-tests
   `ironplcvmd` today.

## Verification

- `cd compiler && just` (compile, coverage, lint, dupes).
- `shellcheck -s sh compiler/install.sh` — what CI's lint-install-script job runs.
- `just install-script-smoke` against v0.235.0: warns about `ironplcvmd`,
  installs the rest, and is idempotent on the second run.
- A synthetic archive containing all four binaries installs all four.

## Not delivered

Removing `ironplcvmd` from `LEGACY_OPTIONAL_BINARIES`. That is a one-line
follow-up for the first release whose tarball carries it; the comment on the
list says so.
