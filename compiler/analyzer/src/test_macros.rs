//! Declarative macros for analyzer rule tests.
//!
//! Every `rule_*.rs` rule has an inline `#[cfg(test)] mod tests` whose cases all
//! repeat the same scaffold: build a library + semantic context from an IEC
//! 61131-3 program string, call the rule's `apply`, and assert the result is
//! `Ok`/`Err` (optionally checking a specific [`Problem`](ironplc_problems::Problem)
//! code and diagnostic count). That 4–8 line body, repeated hundreds of times
//! across ~20 rule files, is what `cargo dupes` flags as the analyzer's largest
//! duplicate mass.
//!
//! These macros collapse each case to a single line while preserving its exact
//! semantics. Each invocation expands to one BDD-named `#[test] fn`, and any
//! `#[…]`/`///` attribute placed before the invocation is forwarded onto it.
//!
//! Two families, differing only in how the semantic context is built:
//!
//! * `rule_ok!` / `rule_err!` / `rule_err_code!` / `rule_err1!` /
//!   `rule_err1_at!` (+ `_with` options variants) — the "fresh context" scaffold
//!   ([`resolve_fresh_with`](crate::test_helpers::resolve_fresh_with)): the
//!   resolved context is discarded and a fresh empty one is used. The same
//!   options value is threaded into both resolution and `apply`.
//! * `rule_ctx_ok!` / `rule_ctx_err!` / `rule_ctx_err_code!` / `rule_ctx_err1!`
//!   — the "resolved context" scaffold
//!   ([`parse_and_resolve_types_with_context`](crate::test_helpers::parse_and_resolve_types_with_context)),
//!   always with default options.
//!
//! `apply` is referenced as `super::apply`, which resolves to the owning rule's
//! function at each invocation site (the macros are invoked inside `rule_X::tests`).

// --- Fresh-context family (options-parameterised) ---------------------------

/// A rule test that expects `Ok` under `$opts` (used for both resolution and apply).
macro_rules! rule_ok_with {
    ($(#[$m:meta])* $name:ident, $opts:expr, $program:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let opts = $opts;
            let (library, context) = $crate::test_helpers::resolve_fresh_with($program, &opts);
            assert!(super::apply(&library, &context, &opts).is_ok());
        }
    };
}

/// A rule test that expects `Ok` under default options.
macro_rules! rule_ok {
    ($(#[$m:meta])* $name:ident, $program:expr $(,)?) => {
        rule_ok_with!(
            $(#[$m])* $name,
            ironplc_parser::options::CompilerOptions::default(),
            $program
        );
    };
}

/// A rule test that expects `Err` (any diagnostics) under `$opts`.
macro_rules! rule_err_with {
    ($(#[$m:meta])* $name:ident, $opts:expr, $program:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let opts = $opts;
            let (library, context) = $crate::test_helpers::resolve_fresh_with($program, &opts);
            assert!(super::apply(&library, &context, &opts).is_err());
        }
    };
}

/// A rule test that expects `Err` (any diagnostics) under default options.
macro_rules! rule_err {
    ($(#[$m:meta])* $name:ident, $program:expr $(,)?) => {
        rule_err_with!(
            $(#[$m])* $name,
            ironplc_parser::options::CompilerOptions::default(),
            $program
        );
    };
}

/// A rule test that expects `Err` including a diagnostic with `$problem`'s code,
/// under `$opts`.
macro_rules! rule_err_code_with {
    ($(#[$m:meta])* $name:ident, $opts:expr, $program:expr, $problem:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let opts = $opts;
            let (library, context) = $crate::test_helpers::resolve_fresh_with($program, &opts);
            let errors = super::apply(&library, &context, &opts).unwrap_err();
            assert!(
                errors.iter().any(|d| d.code == $problem.code()),
                "expected a {} diagnostic, got {:?}",
                $problem.code(),
                errors
            );
        }
    };
}

/// A rule test that expects `Err` including a diagnostic with `$problem`'s code,
/// under default options.
macro_rules! rule_err_code {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr $(,)?) => {
        rule_err_code_with!(
            $(#[$m])* $name,
            ironplc_parser::options::CompilerOptions::default(),
            $program, $problem
        );
    };
}

/// A rule test that expects exactly one diagnostic, with `$problem`'s code,
/// under `$opts`.
macro_rules! rule_err1_with {
    ($(#[$m:meta])* $name:ident, $opts:expr, $program:expr, $problem:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let opts = $opts;
            let (library, context) = $crate::test_helpers::resolve_fresh_with($program, &opts);
            let errors = super::apply(&library, &context, &opts).unwrap_err();
            assert_eq!(errors.len(), 1, "expected exactly one diagnostic, got {:?}", errors);
            assert_eq!(errors[0].code, $problem.code());
        }
    };
}

/// A rule test that expects exactly one diagnostic, with `$problem`'s code,
/// under default options.
macro_rules! rule_err1 {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr $(,)?) => {
        rule_err1_with!(
            $(#[$m])* $name,
            ironplc_parser::options::CompilerOptions::default(),
            $program, $problem
        );
    };
}

/// A rule test that expects exactly one diagnostic, with `$problem`'s code,
/// whose primary label points exactly at `$at` -- the first occurrence of that
/// text in `$program`.
///
/// Use this instead of `rule_err1!` for a rule whose diagnostic is only
/// actionable if it names *where* the offending construct is. The `is_err`
/// assertions above hold just as well when the label carries a default
/// `SourceSpan` -- `range(0, 0)`, which renders as a caret on the first
/// character of the file -- so they cannot catch a span that was never
/// filled in.
macro_rules! rule_err1_at {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr, $at:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let opts = ironplc_parser::options::CompilerOptions::default();
            let (library, context) = $crate::test_helpers::resolve_fresh_with($program, &opts);
            let errors = super::apply(&library, &context, &opts).unwrap_err();
            assert_eq!(errors.len(), 1, "expected exactly one diagnostic, got {:?}", errors);
            assert_eq!(errors[0].code, $problem.code());

            let start = $program
                .find($at)
                .expect("the expected text does not occur in the program");
            let location = &errors[0].primary.location;
            assert_eq!(
                (location.start, location.end),
                (start, start + $at.len()),
                "expected the label to point at {:?}, but it points at {:?}",
                $at,
                $program.get(location.start..location.end),
            );
        }
    };
}

// --- Resolved-context family (default options) ------------------------------

/// A rule test (resolved context, default options) that expects `Ok`.
macro_rules! rule_ctx_ok {
    ($(#[$m:meta])* $name:ident, $program:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let (library, context) =
                $crate::test_helpers::parse_and_resolve_types_with_context($program);
            assert!(super::apply(
                &library,
                &context,
                &ironplc_parser::options::CompilerOptions::default()
            )
            .is_ok());
        }
    };
}

/// A rule test (resolved context, default options) that expects `Err`.
macro_rules! rule_ctx_err {
    ($(#[$m:meta])* $name:ident, $program:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let (library, context) =
                $crate::test_helpers::parse_and_resolve_types_with_context($program);
            assert!(super::apply(
                &library,
                &context,
                &ironplc_parser::options::CompilerOptions::default()
            )
            .is_err());
        }
    };
}

/// A rule test (resolved context, default options) that expects `Err` including
/// a diagnostic with `$problem`'s code.
macro_rules! rule_ctx_err_code {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let (library, context) =
                $crate::test_helpers::parse_and_resolve_types_with_context($program);
            let errors = super::apply(
                &library,
                &context,
                &ironplc_parser::options::CompilerOptions::default(),
            )
            .unwrap_err();
            assert!(
                errors.iter().any(|d| d.code == $problem.code()),
                "expected a {} diagnostic, got {:?}",
                $problem.code(),
                errors
            );
        }
    };
}

/// A rule test (resolved context, default options) that expects exactly one
/// diagnostic, with `$problem`'s code.
macro_rules! rule_ctx_err1 {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let (library, context) =
                $crate::test_helpers::parse_and_resolve_types_with_context($program);
            let errors = super::apply(
                &library,
                &context,
                &ironplc_parser::options::CompilerOptions::default(),
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1, "expected exactly one diagnostic, got {:?}", errors);
            assert_eq!(errors[0].code, $problem.code());
        }
    };
}
