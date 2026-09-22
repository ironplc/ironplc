//! Spec conformance tests for behavior policies (parser-owned requirements):
//! the option surface and the dialect presets.
//!
//! Each test is annotated with `#[spec_test(REQ_BP_parser_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every parser-owned requirement has a test.
//!
//! See `specs/design/behavior-policies.md`.

use rstest::rstest;
use spec_test_macro::spec_test;

use crate::options::{
    CompilerOptions, Dialect, SetPolicyError, StringToNumFailure, StringToNumNonNumeric,
};

fn selected(options: &CompilerOptions) -> (StringToNumNonNumeric, StringToNumFailure) {
    (
        options.policy_string_to_num_non_numeric,
        options.policy_string_to_num_failure,
    )
}

/// REQ-BP-parser-001: the default options and the strict dialects select
/// `reject` and `trap`.
#[spec_test(REQ_BP_parser_001)]
#[rstest]
#[case::default_options(CompilerOptions::default())]
#[case::ed2(CompilerOptions::from_dialect(Dialect::Iec61131_3Ed2))]
#[case::ed3(CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3))]
fn parser_spec_req_bp_001_strict_selects_reject_and_trap(#[case] options: CompilerOptions) {
    assert_eq!(
        selected(&options),
        (StringToNumNonNumeric::Reject, StringToNumFailure::Trap)
    );
}

/// REQ-BP-parser-002: the vendor presets select the surveyed behaviors.
#[spec_test(REQ_BP_parser_002)]
#[rstest]
#[case::rusty(
    Dialect::Rusty,
    StringToNumNonNumeric::Reject,
    StringToNumFailure::Zero
)]
#[case::codesys(
    Dialect::Codesys,
    StringToNumNonNumeric::IgnoreTrailing,
    StringToNumFailure::Zero
)]
#[case::twincat(
    Dialect::TwinCat,
    StringToNumNonNumeric::IgnoreTrailing,
    StringToNumFailure::Zero
)]
fn parser_spec_req_bp_002_vendor_presets_select_surveyed_behavior(
    #[case] dialect: Dialect,
    #[case] non_numeric: StringToNumNonNumeric,
    #[case] failure: StringToNumFailure,
) {
    assert_eq!(
        selected(&CompilerOptions::from_dialect(dialect)),
        (non_numeric, failure)
    );
}

/// REQ-BP-parser-003: `set_policy_by_key` selects by key and CLI name, and
/// rejects an unknown key or alternative without changing anything.
#[spec_test(REQ_BP_parser_003)]
fn parser_spec_req_bp_003_set_policy_by_key_selects_or_rejects() {
    let mut options = CompilerOptions::from_dialect(Dialect::Codesys);
    for pd in CompilerOptions::POLICY_DESCRIPTORS {
        for alt in pd.alternatives {
            assert_eq!(options.set_policy_by_key(pd.option_key, alt), Ok(()));
            assert_eq!(options.get_policy_by_key(pd.option_key), Some(*alt));
        }
    }
    let before = selected(&options);
    assert_eq!(
        options.set_policy_by_key("policy_string_to_num_failure", "wrap"),
        Err(SetPolicyError::UnknownAlternative)
    );
    assert_eq!(
        options.set_policy_by_key("policy_nonexistent", "trap"),
        Err(SetPolicyError::UnknownKey)
    );
    assert_eq!(selected(&options), before);
}

/// REQ-BP-parser-004: `POLICY_DESCRIPTORS` names every policy with its flag,
/// key, alternatives in encoding order, and default.
#[spec_test(REQ_BP_parser_004)]
fn parser_spec_req_bp_004_policy_descriptors_describe_every_policy() {
    let keys: Vec<&str> = CompilerOptions::POLICY_DESCRIPTORS
        .iter()
        .map(|pd| pd.option_key)
        .collect();
    assert_eq!(
        keys,
        vec![
            "policy_string_to_num_non_numeric",
            "policy_string_to_num_failure"
        ]
    );
    let defaults = CompilerOptions::default();
    for pd in CompilerOptions::POLICY_DESCRIPTORS {
        assert!(pd.cli_flag.starts_with("--policy-"));
        assert_eq!(
            pd.cli_flag.trim_start_matches("--").replace('-', "_"),
            pd.option_key
        );
        assert_eq!(pd.default, pd.alternatives[0]);
        assert_eq!(defaults.get_policy_by_key(pd.option_key), Some(pd.default));
    }
    let non_numeric = &CompilerOptions::POLICY_DESCRIPTORS[0];
    assert_eq!(
        non_numeric.alternatives,
        &["reject", "ignore-trailing", "ignore-surrounding"]
    );
    let failure = &CompilerOptions::POLICY_DESCRIPTORS[1];
    assert_eq!(failure.alternatives, &["trap", "zero"]);
}
