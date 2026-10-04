//! Fails when a token rule's tests break the rule-test conventions in
//! `specs/steering/compiler-standards.md` (Rule Tests). See
//! [`ironplc_test::rule_conventions`] for the checks and how a line opts out.
//!
//! Token rules take tokens, not a library and context, so the checks on
//! macros, contexts and the analyze pipeline do not apply.

use ironplc_test::rule_conventions::{violations, Convention};
use std::path::Path;

const CONVENTIONS: &[Convention] = &[
    Convention::StringCode,
    Convention::AnyError,
    Convention::MissingOk,
    Convention::MissingErr,
];

#[test]
fn rule_tests_when_checked_then_follow_conventions() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    let found: Vec<String> = violations(&src, CONVENTIONS)
        .iter()
        .map(ToString::to_string)
        .collect();

    assert!(
        found.is_empty(),
        "rule tests break the conventions in specs/steering/compiler-standards.md (Rule Tests):\n{}",
        found.join("\n")
    );
}
