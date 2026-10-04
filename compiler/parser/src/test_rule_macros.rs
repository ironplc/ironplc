//! Declarative macros for parser token rule tests.
//!
//! Each macro expands to one `#[test] fn` that runs the owning rule's `apply`
//! over a token stream and asserts exactly what it reports. Any `#[…]`/`///`
//! attribute placed before the test name is forwarded onto the generated
//! function.
//!
//! * `token_rule_ok!(name, tokens)`: the rule reports nothing.
//! * `token_rule_err!(name, tokens, [P])`: the rule reports exactly the listed
//!   problems, in order: `[P]`, `[P, Q, ...]`, or `[P; n]`.
//!
//! `tokens` is any expression giving the tokens, typically built with
//! [`token`](crate::test_rule_helpers::token). Each macro takes an optional
//! trailing `CompilerOptions` argument; it defaults to
//! `CompilerOptions::default()`.
//!
//! `apply` is referenced as `super::apply`, which resolves to the owning rule's
//! function at each invocation site (the macros are invoked inside `rule_X::test`).

/// A token rule test that expects the rule to report no problems for
/// `$tokens`, under `$opts` (default options when omitted).
macro_rules! token_rule_ok {
    ($(#[$m:meta])* $name:ident, $tokens:expr, $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let tokens = $tokens;
            let result = super::apply(&tokens, &$opts);
            let codes = $crate::test_rule_helpers::result_codes(&result);
            assert!(codes.is_empty(), "{codes:?}");
        }
    };
    ($(#[$m:meta])* $name:ident, $tokens:expr $(,)?) => {
        token_rule_ok!(
            $(#[$m])* $name,
            $tokens,
            $crate::options::CompilerOptions::default()
        );
    };
}

/// A token rule test that expects the rule to report exactly the listed
/// problems for `$tokens`, in order, under `$opts` (default options when
/// omitted). The list is `[P]`, `[P, Q, ...]`, or `[P; n]`.
macro_rules! token_rule_err {
    ($(#[$m:meta])* $name:ident, $tokens:expr, [$problem:expr; $count:expr], $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let tokens = $tokens;
            let result = super::apply(&tokens, &$opts);
            let codes = $crate::test_rule_helpers::result_codes(&result);
            assert_eq!(codes, [$problem.code(); $count]);
        }
    };
    ($(#[$m:meta])* $name:ident, $tokens:expr, [$($problem:expr),+ $(,)?], $opts:expr $(,)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            let tokens = $tokens;
            let result = super::apply(&tokens, &$opts);
            let codes = $crate::test_rule_helpers::result_codes(&result);
            assert_eq!(codes, [$($problem.code()),+]);
        }
    };
    ($(#[$m:meta])* $name:ident, $tokens:expr, [$($list:tt)+] $(,)?) => {
        token_rule_err!(
            $(#[$m])* $name,
            $tokens,
            [$($list)+],
            $crate::options::CompilerOptions::default()
        );
    };
}
