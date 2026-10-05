//! Declarative macros for analyzer rule tests.
//!
//! Each macro expands to one BDD-named `#[test] fn` that resolves an IEC
//! 61131-3 program, runs the owning rule's `apply` against the resolved
//! library and context (see [`rule_codes`](crate::test_helpers::rule_codes)),
//! and asserts exactly what it reports. Any `#[…]`/`///` attribute placed
//! before the test name is forwarded onto the generated function.
//!
//! * `rule_ok!(name, program)`: the rule reports nothing.
//! * `rule_err!(name, program, [P])`: the rule reports exactly the listed
//!   problems, in order: `[P]`, `[P, Q, ...]`, or `[P; n]`.
//! * `rule_err_at!(name, program, P, "text")`: the rule reports one `P`,
//!   labelled at the first occurrence of `"text"`.
//!
//! Each takes an optional trailing `CompilerOptions` argument, used both to
//! resolve the program and to run the rule; it defaults to
//! `CompilerOptions::default()`.
//!
//! `apply` is referenced as `super::apply`, which resolves to the owning rule's
//! function at each invocation site (the macros are invoked inside `rule_X::tests`).

/// A rule test that expects the rule to report no problems for `$program`,
/// under `$opts` (default options when omitted).
macro_rules! rule_ok {
    ($(#[$m:meta])* $name:ident, $program:expr, $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let codes = $crate::test_helpers::rule_codes(super::apply, $program, &$opts);
            assert!(codes.is_empty(), "{codes:?}");
        }
    };
    ($(#[$m:meta])* $name:ident, $program:expr $(,)?) => {
        rule_ok!(
            $(#[$m])* $name,
            $program,
            ironplc_parser::options::CompilerOptions::default()
        );
    };
}

/// A rule test that expects the rule to report exactly the listed problems for
/// `$program`, in order, under `$opts` (default options when omitted). The
/// list is `[P]`, `[P, Q, ...]`, or `[P; n]` for `n` reports of one problem.
macro_rules! rule_err {
    ($(#[$m:meta])* $name:ident, $program:expr, [$problem:expr; $count:expr], $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let codes = $crate::test_helpers::rule_codes(super::apply, $program, &$opts);
            assert_eq!(codes, [$problem.code(); $count]);
        }
    };
    ($(#[$m:meta])* $name:ident, $program:expr, [$($problem:expr),+ $(,)?], $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let codes = $crate::test_helpers::rule_codes(super::apply, $program, &$opts);
            assert_eq!(codes, [$($problem.code()),+]);
        }
    };
    ($(#[$m:meta])* $name:ident, $program:expr, [$($list:tt)+] $(,)?) => {
        rule_err!(
            $(#[$m])* $name,
            $program,
            [$($list)+],
            ironplc_parser::options::CompilerOptions::default()
        );
    };
}

/// A rule test that expects the rule to report exactly one `$problem` for
/// `$program`, under `$opts` (default options when omitted), whose primary
/// label points exactly at `$at`: the first occurrence of that text in
/// `$program`.
///
/// Use this for a rule whose diagnostic is only actionable if it names
/// *where* the offending construct is: an assertion on codes alone holds just
/// as well when the label carries a default `SourceSpan`, which renders as a
/// caret on the first character of the file.
macro_rules! rule_err_at {
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr, $at:expr, $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let program: &str = $program;
            let diagnostics =
                $crate::test_helpers::rule_diagnostics(super::apply, program, &$opts);
            assert_eq!(
                $crate::test_helpers::diagnostic_codes(&diagnostics),
                [$problem.code()]
            );

            let start = program
                .find($at)
                .expect("the expected text does not occur in the program");
            let location = &diagnostics[0].primary.location;
            assert_eq!(
                (location.start, location.end),
                (start, start + $at.len()),
                "{:?}",
                program.get(location.start..location.end),
            );
        }
    };
    ($(#[$m:meta])* $name:ident, $program:expr, $problem:expr, $at:expr $(,)?) => {
        rule_err_at!(
            $(#[$m])* $name,
            $program,
            $problem,
            $at,
            ironplc_parser::options::CompilerOptions::default()
        );
    };
}
