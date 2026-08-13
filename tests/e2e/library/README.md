# Library installer end-to-end test

This fixture backs the `library-e2e` test, which verifies that the **installers
actually ship the bundled compatibility libraries** and that the installed
toolchain resolves library symbols from beside the binary.

The compiler reads bundled libraries from `resources/libs` next to its
executable. Unit tests can't catch a packaging miss because they fall back to the
crate source tree; only a real install can. `uses_pi.st` is a minimal program
that depends on the `Tc2_System` library's `PI` constant — it computes
`2 * PI * 10.0` — so it compiles and runs **only** when the installer placed
`resources/libs/Tc2_System/...` beside the binary.

The test installs with the real OS installer, then:

1. compiles the program with the installed `ironplcc` (`--library Tc2_System`),
2. runs one scan on the installed `ironplcvm` (`--scans 1 --dump-vars -`),
3. asserts the dump shows `circumference` = `62.83185307179586`.

That end-to-end path proves the library `PI` resolved from the installed location
and the VM computed with it.

The same flow then repeats for `uses_bool_to_string.st` with
`--library Tc2_BuiltIns`, asserting `okTrue`/`okFalse` are both `TRUE` — proving
the `Tc2_BuiltIns` library's `BOOL_TO_STRING` function body shipped, resolved,
and executed correctly (`--dump-vars` does not render STRING contents, so the
fixture folds the results into BOOLs via string comparison). The target release
must ship both bundled libraries.

## One verification, three installers

`verify.sh` is the whole test: it takes an installed `ironplcc` and `ironplcvm`,
does the compile/run/assert above, and is the *only* place those assertions
exist. Every platform runs the same script — Windows through Git for Windows'
`bash` — so the per-OS `just` recipes shrink to one line naming the two binary
paths.

That is deliberate. The assertions previously existed twice, once in `sh` and
once in PowerShell, and the copies drifted: PowerShell captures multi-line
command output as an *array*, where `-notmatch` filters instead of returning a
boolean, so the Windows leg reported a correct result as a failure while Linux
stayed green. Keep new assertions in `verify.sh`, never in a recipe.

What genuinely cannot be shared is the install itself — tarball, NSIS, and
Homebrew put files in different places, and that difference is the thing under
test. Everything after the install is common.

`verify.sh` hands the toolchain only *relative* paths (`tests/e2e/library/...`,
`target/library-e2e/...`), because a native Windows `.exe` does not understand
POSIX paths such as `mktemp -d`'s `/tmp/tmp.XXXX`. Keep it that way.

## Running it

Each OS installs differently, so there are per-OS recipes. The release version is
required (no "latest" resolution) and must be one that ships the libraries:

```sh
# Linux / macOS — tarball + install.sh
just library-e2e 0.234.0

# Windows — NSIS installer
just library-e2e 0.234.0

# macOS — Homebrew formula (libexec layout; separate from the tarball path)
just library-e2e-brew 0.234.0
```

Like the VS Code `endtoend-smoke` test, each is split into a download step and a
test step, so you can install once and re-run the verification repeatedly:

```sh
just library-e2e-download 0.234.0   # acquire + install (tarball/NSIS)
just library-e2e-test               # compile + run + verify (repeatable)

just library-e2e-brew-download 0.234.0   # acquire + install (Homebrew)
just library-e2e-brew-test               # compile + run + verify (repeatable)
```

Or drive the verification directly against any installed toolchain:

```sh
sh tests/e2e/library/verify.sh ~/.ironplc/bin/ironplcc ~/.ironplc/bin/ironplcvm
```

In CI this is `partial_library_e2e.yaml`: a Linux/macOS/Windows matrix for the
tarball + NSIS installers, plus a dedicated macOS job for the Homebrew
installer. It runs as part of `deployment.yaml` and gates `publish-website`, so
a release that fails to ship the bundled libraries cannot be published. Because
the recipes can only be exercised on a real runner, change them on a branch and
validate with the workflow's `workflow_dispatch` trigger from the Actions tab
before merging.
