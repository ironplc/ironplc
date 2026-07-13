# Plan: Ensure P9xxx diagnostics always carry a compiler source location

## Problem

The PostHog "Top P9 compiler error locations" dashboard breaks failed
`compile_finished` events down by the `error_locations` property. That property
is populated only when a `Diagnostic` carries `source_file` / `source_line`
(the compiler's own `file#Lline`, set via `Diagnostic::with_source`). Those
fields are set in only two places today:

- `Diagnostic::todo*()` (P9999 — NotImplemented)
- `Diagnostic::internal_error()` (P9998 — InternalError)

Every diagnostic built with the plain `Diagnostic::problem(Problem::X, label)`
constructor leaves those fields `None`, so it lands in PostHog's blank
`(none)` bucket. Concretely, these P9 diagnostics have **no** compiler
location:

- **56** `Diagnostic::problem(Problem::NotImplemented, …)` sites (mostly in
  `codegen/`) — P9999 raised without going through `todo*()`.
- **6** `Diagnostic::problem(Problem::InternalError, …)` sites — P9998 raised
  without going through `internal_error()`.
- P9001 / P9002 / P9003 — separate follow-up; these are analyzer/parser codes
  whose "location" is the program, not the compiler, so they are out of scope
  for this change.

Because P9999 is the most common P9 code and its codegen sites bypass the
location-capturing helpers, most P9999 hits show up location-less — defeating
the entire purpose of the dashboard tile (pointing maintainers at the code
that needs work).

## Goals

1. Every P9998/P9999 diagnostic carries the compiler `file#Lline` that produced
   it, while preserving the meaningful primary label each site sets today.
2. Make it a **compile error** to construct P9998/P9999 via the raw
   `Diagnostic::problem(...)` path, so this cannot regress.

## Design

### New constructors (`dsl/src/diagnostic.rs`)

Add two `#[track_caller]` constructors that capture the caller's location
automatically (via `std::panic::Location::caller()` — no `unsafe`, no
`file!()`/`line!()` plumbing at call sites) and accept the site's own label:

```rust
#[track_caller]
pub fn not_implemented(primary: Label) -> Self { … }   // P9999

#[track_caller]
pub fn internal_error_at(primary: Label) -> Self { … } // P9998
```

Both set `source_file`/`source_line` from `Location::caller()`. The existing
`todo*()` / `internal_error(file, line)` helpers stay (they hardcode a generic
message and are still used by ~65 sites).

### Convert the raw sites

- 56 `Diagnostic::problem(Problem::NotImplemented, L)` → `Diagnostic::not_implemented(L)`
- 6 `Diagnostic::problem(Problem::InternalError, L)` → `Diagnostic::internal_error_at(L)`

### Prevention (make the footgun not compile)

- In `problems/build.rs`, emit `#[deprecated(note = …)]` on the generated
  `NotImplemented` and `InternalError` enum variants (hardcoded name set in the
  build script; the CSV schema is untouched so other CSV readers are
  unaffected).
- Add `deprecated = "deny"` to `[workspace.lints.rust]`. Direct use of either
  variant is now a hard compile error pointing at the sanctioned constructors.
- Add `#[allow(deprecated)]` on the sanctioned internal references: the
  generated `impl Problem` (code/message), the `todo*()` / `internal_error()` /
  `not_implemented()` / `internal_error_at()` constructors, and the handful of
  test assertions that name the variants (or switch them to the string code).

### Docs

Update `specs/steering/problem-code-management.md` to state that P9998/P9999
must be created via the dedicated constructors.

## Validation

`cd compiler && just` (compile + coverage/tests + clippy/fmt). Add a unit test
asserting `not_implemented(...)` / `internal_error_at(...)` populate
`source_file`/`source_line`.
