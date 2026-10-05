//! Fails when a rule's tests break the rule-test conventions in
//! `specs/steering/compiler-standards.md` (Rule Tests). See
//! [`ironplc_test::rule_conventions`] for the checks and how a line opts out.

use ironplc_test::rule_conventions::{unregistered_rules, violations, Convention};
use std::path::Path;

#[test]
fn rule_tests_when_checked_then_follow_conventions() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    let found: Vec<String> = violations(&src, Convention::ALL)
        .iter()
        .map(ToString::to_string)
        .collect();

    assert!(
        found.is_empty(),
        "rule tests break the conventions in specs/steering/compiler-standards.md (Rule Tests):\n{}",
        found.join("\n")
    );
}

#[test]
fn rules_when_checked_then_each_runs_in_the_pipeline() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    let unregistered = unregistered_rules(&src, &src.join("stages.rs"));

    assert!(
        unregistered.is_empty(),
        "rules whose apply the analysis stages (stages.rs) never calls: {unregistered:?}"
    );
}
