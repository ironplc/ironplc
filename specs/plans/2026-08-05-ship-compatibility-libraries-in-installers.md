# Plan: Ship compatibility-library files in the installers

## Goal

Close a packaging gap left by the [Compatibility Libraries](../design/compatibility-libraries.md)
increment: the runtime loader reads bundled libraries from `resources/libs`
*beside the compiler executable*
(`compiler/sources/src/libraries/mod.rs::installed_libraries_root`), but **no
distribution artifact ships that directory**. Every real install therefore fails
to resolve library symbols — most visibly, `PI` for TwinCAT — even though it
works in dev/test via the `CARGO_MANIFEST_DIR` fallback.

The fix is to ship `compiler/sources/resources/libs/` in each distribution so it
lands at `<bindir>/resources/libs`, exactly where the loader looks.

## Background

`installed_libraries_root()` prefers `<exe_dir>/resources/libs` and only falls
back to the crate source tree when that directory is absent (development/test).
The four distribution paths and where each puts the executable:

| Distribution        | Executable location        | Loader expects                    |
|---------------------|----------------------------|-----------------------------------|
| Linux/macOS tarball | `$INSTALL_DIR/bin/`        | `$INSTALL_DIR/bin/resources/libs` |
| Windows NSIS        | `$INSTDIR\bin\`            | `$INSTDIR\bin\resources\libs`     |
| Homebrew            | `libexec/` (symlinked)     | `#{libexec}/resources/libs`       |

`current_exe()` resolves symlinks, so a `bin` symlink on `PATH` resolves back to
the real binary. The idiomatic Homebrew layout keeps the binaries and their
resources together in `libexec` and symlinks the executables onto the `PATH`, so
the loader finds `#{libexec}/resources/libs` beside the resolved binary.

**Contract:** libraries install to `<bindir>/resources/libs`, uniformly across
all three installers. The design doc's *Installation* section previously left the
install location "defined separately when needed"; it is now needed and this plan
defines it.

## Non-goals

- Changing the loader search logic or the on-disk package format (unchanged).
- A user-facing library-install command or an external library search path.
- Shipping any new library; only the already-bundled `Tc2_System` travels.

## File map

**Modified — packaging**
- `compiler/justfile` — `_package-linux` and `_package-macos` stage
  `sources/resources/libs` into the release dir and add `resources/` to the
  tarball. (Windows reads the source tree directly from `setup.nsi`.)
- `compiler/setup.nsi` — install `resources\libs` into `$INSTDIR\bin\resources`.
- `compiler/install.sh` — place the extracted `resources/` dir next to the
  installed binaries; warn (not fail) when an older release omits it.
- `compiler/homebrew/Formula/ironplc.rb` — install the binaries and
  `resources/libs` into `libexec` and symlink the executables into `bin`.

**Modified — verification**
- `justfile` (root) — `_install-script-smoke-verify` asserts the library files
  shipped and that a `PI`-using program compiles with `--library Tc2_System`.

**Modified — docs**
- `specs/design/compatibility-library-format.md` — define the install location
  (`<bindir>/resources/libs`) in the *Installation* section.

## Testing strategy

- `cd compiler && just` green (compile, coverage ≥85%, clippy, fmt, dupes).
- Local tarball assembly check: run the `tar` step and assert the archive
  contains `resources/libs/Tc2_System/library.toml`.
- The install-script smoke test (CI, Linux + macOS) additionally asserts, after a
  real install, that `resources/libs/Tc2_System/library.toml` exists beside the
  binaries and that `ironplcc check --library Tc2_System` compiles a program that
  reads `PI`. This exercises the *shipped* artifact, not the dev fallback.

## Tasks

- [x] Stage `resources/libs` into the tarball (`_package-linux`, `_package-macos`)
- [x] Install `resources\libs` from `setup.nsi` (Windows)
- [x] Place `resources/` beside the binaries in `install.sh`
- [x] Install `resources/libs` in the Homebrew formula
- [x] Assert shipped files + `PI` compile in the install-script smoke verify
- [x] Define the install location in the format design doc
- [x] `cd compiler && just` green
