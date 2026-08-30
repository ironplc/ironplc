# Reject a non-positive `scanLimit` in the debug server

## Motivation

[#1515](https://github.com/ironplc/ironplc/issues/1515): two shipped surfaces
tell the user that `scanLimit: 0` means unlimited —
`docs/reference/runtime/ironplcvmd.rst` and the VS Code launch-configuration
schema in `integrations/vscode/package.json` — while the server does the
opposite. `scan_limit_reached` compares `scan_count() >= limit`, so `Some(0)`
is true at the first completed scan and the session terminates after a single
cycle: the most restrictive setting possible, reached by typing the value
documented as the least restrictive.

The issue offers two fixes. This plan takes the second: **reject `0` rather
than reinterpret it**, and drop the "0 means unlimited" claim everywhere.

The reason is that "unlimited" already has a spelling — omit the argument. The
server models the bound as `Option<u64>` precisely so absence carries that
meaning, and `None` is what a launch config without `scanLimit` produces
already. Adding `Some(0)` as a second spelling of the same thing gives the
type two representations of one state and leaves `0` as a sentinel a reader
has to know about. A user who writes `scanLimit: 0` is far more likely to have
meant "no bound" than "no scans", but they are also just as likely to be
guessing; an error that says so teaches the right spelling, where a silent
reinterpretation hides that there was ever a question.

The same argument rejects a negative limit. `-1` is the other conventional
"unlimited" sentinel and today it fails `Option<u64>` deserialization, which
fails the *whole* `LaunchRequestArguments` parse, which the server can only
report as `V6008 - launch requires a 'program' path` — a message about an
argument the user did supply. That misreport is worth fixing on its own.

## Prefactoring

None needed. The change adds a validation step on a path that has one
already (`load_and_check` → `launch::check_preconditions`) and fits the shape
of the existing `LaunchError` variants. The one structural move — holding the
raw `scanLimit` as a `serde_json::Number` so validation can see what the client
sent — is required by the change, not a cleanup that precedes it.

## Changes

1. **Raw argument** (`compiler/vm-cli/src/dap/types.rs`): change
   `LaunchRequestArguments::scan_limit` from `Option<u64>` to
   `Option<serde_json::Number>`. `u64` rejects a negative or fractional value
   during deserialization, and because `load_and_check` parses with `.ok()`
   that failure is reported as a missing `program`. Holding the number lets
   validation report the scan-limit problem it actually is.

2. **New error** (`compiler/vm-cli/src/dap/launch.rs`):
   `LaunchError::ScanLimitNotPositive(String)`, carrying the value as the
   client wrote it, with problem code `V6011` and a message that names the
   fix ("omit `scanLimit` to run without a bound").

3. **Validation** (`compiler/vm-cli/src/dap/launch.rs`):
   `check_scan_limit(Option<&Number>) -> Result<Option<NonZeroU64>, LaunchError>`.
   `None` stays `None` (no bound). A present value must be a whole number of
   at least 1; zero, negative and fractional values are rejected. Returning
   `NonZeroU64` makes "a bound of zero scans" unrepresentable downstream
   rather than merely unreachable.

4. **Wiring** (`compiler/vm-cli/src/dap/server.rs`): `load_and_check` runs the
   validation alongside the container preconditions and returns the validated
   `Option<NonZeroU64>`, so a bad limit is answered as a failed `launch`
   before the VM starts. `launched_session` takes the validated bound and
   `scan_limit_reached` compares against `limit.get()`.

5. **Problem code** (`compiler/vm-cli/resources/problem-codes.csv`):
   `V6011,LaunchInvalidScanLimit,...`, and the page
   `docs/reference/runtime/problems/V6011.rst`. The page must clear the 900
   character article-text minimum enforced by `check_problem_page_lengths`;
   new pages are not added to the thin-page allowlist.

6. **Documentation** — remove the "``0`` means unlimited" claim and say what
   is actually true:
   - `docs/reference/runtime/ironplcvmd.rst` (`launch` argument table)
   - `docs/reference/editor/debugging.rst` (argument table + "Limiting a Run")
   - `integrations/vscode/package.json` (`scanLimit` schema description)
   - `specs/design/debugger-support.md` (the illustrative `package.json`
     schema still shows `"default": 0` / `0 = unlimited`)

## Testing

- `types.rs`: a `scanLimit` of `-1` parses into the arguments struct rather
  than failing the whole parse.
- `launch.rs`: `check_scan_limit` for absent, `1`, `0`, `-1`, and a fractional
  value.
- `server.rs`: `launch` with `scanLimit: 0` and with `scanLimit: -1` are both
  answered with a `V6011` error and no session; `scanLimit: 1` still runs one
  scan and terminates (existing tests cover this).

## Compatibility

A launch configuration carrying `scanLimit: 0` now fails instead of running a
single scan. That is a behaviour change for a setting whose current behaviour
matches neither what the documentation promised nor what any user would want,
and the error names the replacement.
