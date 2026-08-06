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

## Running it

Each OS installs differently, so there are per-OS recipes (empty version = latest;
must be a release that ships the libraries):

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

In CI this is `partial_library_e2e.yaml`, runnable from the Actions tab
(`workflow_dispatch`): a Linux/macOS/Windows matrix for the tarball + NSIS
installers, plus a dedicated macOS job for the Homebrew installer. It is
intentionally not wired into the release or PR pipelines yet.
