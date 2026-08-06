# Library installer end-to-end test

This fixture backs the `library-e2e` test, which verifies that the **installers
actually ship the bundled compatibility libraries** and that the installed
compiler resolves library symbols from beside the binary.

The compiler reads bundled libraries from `resources/libs` next to its
executable. Unit tests can't catch a packaging miss because they fall back to the
crate source tree; only a real install can. `uses_pi.st` depends on the
`Tc2_System` library's `PI` constant, so it compiles **only** when the installer
placed `resources/libs/Tc2_System/...` beside the binary.

## Running it

Against a published release (empty = latest; must be a release that ships the
libraries):

```sh
# Linux / macOS — installs via the tarball + install.sh
just library-e2e 0.234.0

# Windows — installs via the NSIS installer
just library-e2e 0.234.0
```

The recipe installs the real OS installer, asserts the library files landed
beside the binary, then compiles `uses_pi.st` against the installed compiler.

In CI this is `partial_library_e2e.yaml`, runnable from the Actions tab
(`workflow_dispatch`) across Linux, macOS, and Windows. It is intentionally not
wired into the release or PR pipelines yet.
