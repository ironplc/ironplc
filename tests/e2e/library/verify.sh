#!/usr/bin/env sh
# Shared verification for the library installer end-to-end test.
#
# Compiles each library-dependent fixture with an *installed* ironplcc, runs one
# scan on the installed ironplcvm, and asserts the dump shows the results that
# only the bundled compatibility libraries can produce. A release that failed to
# ship a library fails the compile here, so no separate file check is needed.
#
# Usage:
#   sh tests/e2e/library/verify.sh <ironplcc> <ironplcvm>
#
# This is a script rather than a justfile recipe on purpose. The installers
# differ per OS, but this verification does not, and the previous per-OS copies
# of it drifted: the PowerShell copy compared an array of output lines with
# `-notmatch`, which filters instead of returning a boolean, so a correct run was
# reported as a failure. One implementation, run by the same interpreter on every
# platform, is the only way a green Linux run says anything about Windows.
#
# Every path handed to the compiler and VM below is relative, because the callers
# on Windows are native .exe files that do not understand POSIX paths such as
# `mktemp -d`'s /tmp/tmp.XXXX. Relative paths need no translation anywhere. Only
# the two binary paths are absolute, and the Windows caller writes them with
# forward slashes.
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: verify.sh <ironplcc> <ironplcvm>" >&2
  exit 2
fi

ironplcc="$1"
ironplcvm="$2"

# Work from the repository root so the fixture and output paths below are
# relative, however the script was invoked.
cd "$(dirname -- "$0")/../../.."

work="target/library-e2e"
rm -rf "$work"
mkdir -p "$work"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

# compile_and_run <library> <fixture> <name>
#
# Leaves the VM's variable dump in $work/<name>.out and echoes it, so a failing
# run shows what the toolchain actually produced.
compile_and_run() {
  _library="$1"
  _fixture="$2"
  _name="$3"

  "$ironplcc" compile --dialect twincat --library "$_library" \
    --output "$work/$_name.iplc" "$_fixture" \
    || fail "installed compiler could not compile $_fixture against $_library"

  "$ironplcvm" run "$work/$_name.iplc" --scans 1 --dump-vars - > "$work/$_name.out" 2>&1 \
    || { cat "$work/$_name.out"; fail "installed VM could not run $work/$_name.iplc"; }

  cat "$work/$_name.out"
}

echo "Verifying with:"
echo "  ironplcc:  $ironplcc"
echo "  ironplcvm: $ironplcvm"

# Tc2_System: `PI` is a library global, not a language built-in, so after one
# scan `circumference` holds 2 * PI * 10.0 = 62.83185307179586.
compile_and_run Tc2_System tests/e2e/library/uses_pi.st pi
grep -q "62.8318" "$work/pi.out" \
  || fail "VM did not compute 2 * PI * 10.0 from the library PI"
echo "PASS: installed compiler + VM computed 2 * PI * 10.0 using the library PI"

# Tc2_BuiltIns: BOOL_TO_STRING is a library function. --dump-vars does not render
# STRING contents, so the fixture folds the comparisons into BOOLs.
compile_and_run Tc2_BuiltIns tests/e2e/library/uses_bool_to_string.st builtins
grep -q "okTrue: TRUE" "$work/builtins.out" \
  || fail "BOOL_TO_STRING(TRUE) did not return 'TRUE' from the library"
grep -q "okFalse: TRUE" "$work/builtins.out" \
  || fail "BOOL_TO_STRING(FALSE) did not return 'FALSE' from the library"
echo "PASS: installed compiler + VM computed BOOL_TO_STRING using the library"
