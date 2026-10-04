//! Single-scan assertion helpers and the `e2e!` / `e2e_*!` test-declaring
//! macros.

use ironplc_parser::options::CompilerOptions;

use super::run::parse_and_run;
use super::slot_value::{self, NearSlotValue, SlotValue};

/// Runs `source` for one scan and calls `check(index, actual, expected)` for
/// each `(index, expected)` pair.
///
/// Every assertion helper below goes through this function, so it is the one
/// place that reads a variable after a scan.
fn check_each<T: SlotValue>(
    source: &str,
    options: &CompilerOptions,
    asserts: &[(usize, T)],
    check: impl Fn(usize, T, T),
) {
    let (container, bufs) = parse_and_run(source, options);
    for &(idx, expected) in asserts {
        let tag = slot_value::type_tag(&container, idx);
        check(idx, T::from_slot(bufs.vars[idx], tag), expected);
    }
}

/// Runs `source` with `options` and asserts that each `(var_index, expected)`
/// pair matches `vars[var_index]` read as `T` after one scan.
///
/// This is the workhorse helper for the `end_to_end_*.rs` tests: it collapses
/// the recurring 3-line scaffold (`let source ...; let (_c, bufs) =
/// parse_and_run(...); assert_eq!(...)`) into a single call so that each
/// `#[test] fn` becomes one statement. Floating-point values use exact bit
/// equality, so tests must choose inputs that produce deterministic results;
/// use [`assert_run_near`] otherwise.
///
/// Name `T` explicitly (`assert_run_with::<f32>`): an unsuffixed float
/// literal in `asserts` would otherwise default to `f64`.
pub fn assert_run_with<T: SlotValue>(
    source: &str,
    options: &CompilerOptions,
    asserts: &[(usize, T)],
) {
    check_each(source, options, asserts, |idx, actual, expected| {
        assert_eq!(actual, expected, "vars[{idx}] mismatch");
    });
}

/// [`assert_run_with`] with default [`CompilerOptions`].
pub fn assert_run<T: SlotValue>(source: &str, asserts: &[(usize, T)]) {
    assert_run_with(source, &CompilerOptions::default(), asserts);
}

/// Like [`assert_run`] but asserts each value is within `tolerance` of the
/// expected value. Use when arithmetic (pow, transcendentals) produces values
/// that can't be represented exactly.
pub fn assert_run_near<T: NearSlotValue>(source: &str, tolerance: T, asserts: &[(usize, T)]) {
    check_each(
        source,
        &CompilerOptions::default(),
        asserts,
        |idx, actual, expected| {
            assert!(
                actual.distance(expected) < tolerance,
                "vars[{idx}]: expected {expected}, got {actual}"
            );
        },
    );
}

/// Declares a `#[test] fn` like [`e2e_i32`], with the expected type inferred
/// from the values. Use it when they are typed, such as
/// `Duration::seconds(5)` or `date!(2024-01-01)`; an unsuffixed number would
/// default to `i32` or `f64`.
macro_rules! e2e {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run($source, $asserts);
        }
    };
}

/// Declares a `#[test] fn` that asserts an IEC 61131-3 program produces the
/// given i32 var values.
///
/// The macro form (vs writing the `#[test] fn` body directly as
/// `{ assert_run::<i32>(...); }`) matters for code duplication: without it,
/// every short 6-line body gets regrouped by `cargo dupes` as a new
/// exact-duplicate set. A macro invocation is opaque to the detector, so
/// each test becomes a single token and no new group forms.
///
/// Any `#[...]` attributes (including `///` docstrings) preceding the
/// macro invocation are forwarded to the generated `fn`.
///
/// These macros are made visible across the `it` test binary by the
/// `#[macro_use] mod assert;` declaration in `common/mod.rs` and the
/// `#[macro_use] mod common;` declaration in `main.rs`. They reference
/// `$crate::common::...` (re-exported from this module) so they resolve
/// correctly when expanded inside a sibling submodule (e.g.
/// `tests/it/end_to_end_bit_access.rs`).
macro_rules! e2e_i32 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run::<i32>($source, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as i64 (LINT/ULINT).
macro_rules! e2e_i64 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run::<i64>($source, $asserts);
        }
    };
}

/// Like [`e2e_i32`] but takes a [`CompilerOptions`] expression so the test
/// can enable a non-default dialect flag. The options expression is
/// evaluated once inside the generated test body.
macro_rules! e2e_i32_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_with::<i32>($source, &$opts, $asserts);
        }
    };
}

/// Same as [`e2e_i32_with`] but reads slots as i64 (LINT/ULINT).
macro_rules! e2e_i64_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_with::<i64>($source, &$opts, $asserts);
        }
    };
}

/// Same as [`e2e_i32_with`] but reads slots as f32 (REAL).
macro_rules! e2e_f32_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_with::<f32>($source, &$opts, $asserts);
        }
    };
}

/// Same as [`e2e_i32_with`] but reads slots as f64 (LREAL).
macro_rules! e2e_f64_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_with::<f64>($source, &$opts, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as f32 (REAL).
macro_rules! e2e_f32 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run::<f32>($source, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as f64 (LREAL).
macro_rules! e2e_f64 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run::<f64>($source, $asserts);
        }
    };
}

/// Same as [`e2e_f32`] but takes an explicit tolerance. Use when the expected
/// f32 value cannot be represented exactly (e.g. results of `**`, sqrt, ln).
macro_rules! e2e_f32_near {
    ($(#[$meta:meta])* $name:ident, $tol:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_near::<f32>($source, $tol, $asserts);
        }
    };
}

/// Same as [`e2e_f64`] but takes an explicit tolerance.
macro_rules! e2e_f64_near {
    ($(#[$meta:meta])* $name:ident, $tol:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_near::<f64>($source, $tol, $asserts);
        }
    };
}
