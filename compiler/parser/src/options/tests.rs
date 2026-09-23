//! Unit tests for `options`.

use super::*;
use rstest::rstest;
use spec_test_macro::spec_test;

/// Collect the dialect-flag `option_key`s that `from_dialect(dialect)`
/// turns on, sorted for order-independent comparison.
fn enabled_flags(dialect: Dialect) -> Vec<&'static str> {
    let options = CompilerOptions::from_dialect(dialect);
    let mut enabled: Vec<&'static str> = CompilerOptions::FEATURE_DESCRIPTORS
        .iter()
        .filter(|f| options.get_flag_by_key(f.option_key) == Some(true))
        .map(|f| f.option_key)
        .collect();
    enabled.sort_unstable();
    enabled
}

/// Assert that a dialect enables *exactly* the given set of dialect flags --
/// no more, no less. This is the guard against a newly added option
/// silently leaking into a dialect it should not belong to: adding an
/// option to a dialect's macro tags forces a matching update here, and an
/// accidental extra tag makes that dialect's expected set mismatch.
fn assert_enabled_flags(dialect: Dialect, expected: &[&str]) {
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(
        enabled_flags(dialect),
        expected_sorted,
        "dialect {dialect} does not enable exactly the expected dialect flags"
    );
}

/// IEC 61131-3 Ed. 2 (the default) enables no extensions at all.
#[test]
fn ed2_dialect_enables_no_flags() {
    assert_enabled_flags(Dialect::Iec61131_3Ed2, &[]);
}

/// IEC 61131-3 Ed. 3 is a preset assembled from the descriptors tagged with
/// `Iec61131_3Ed3`: the long-time-type keywords, the `REF_TO`/`REF`/`NULL`
/// reference keywords, partial-access syntax, explicit enumeration member
/// values, and the object-oriented syntax (`allow_fb_inheritance`) that is
/// the headline addition of the 2013 edition.
#[test]
fn ed3_dialect_enables_edition3_descriptors() {
    assert_enabled_flags(
        Dialect::Iec61131_3Ed3,
        &[
            "allow_long_time_types",
            "allow_ref_to",
            "allow_partial_access_syntax",
            "allow_fb_inheritance",
            "allow_enum_explicit_values",
        ],
    );
}

/// The RuSTy dialect stays on the Edition-2 keyword base and enables every
/// extension. Listed explicitly (not derived) so a new option that
/// is meant to be Rusty-only, or accidentally left off Rusty, is caught.
#[test]
fn rusty_dialect_enables_exactly_these_flags() {
    assert_enabled_flags(
        Dialect::Rusty,
        &[
            "allow_c_style_comments",
            "allow_missing_semicolon",
            "allow_top_level_var_global",
            "allow_constant_type_params",
            "allow_empty_var_blocks",
            "allow_time_as_function_name",
            "allow_ref_to",
            "allow_ref_arithmetic",
            "allow_ref_stack_variables",
            "allow_ref_type_punning",
            "allow_int_to_bool_initializer",
            "allow_sizeof",
            "allow_system_uptime_global",
            "allow_cross_family_widening",
            "allow_cross_family_conversion",
            "allow_int_literal_to_bit_string",
            "allow_partial_access_syntax",
            "allow_pragmas",
            "allow_short_circuit_operators",
            "allow_mixed_located_var_declarations",
            "allow_constant_initializer_expressions",
            "allow_bit_string_case_labels",
            "allow_paren_string_length",
            "allow_struct_initializer_expressions",
            "allow_fb_inheritance",
            "allow_enum_explicit_values",
            "allow_enum_base_type",
        ],
    );
}

/// The CODESYS dialect is close to RuSTy, with two differences: it does
/// *not* bind the `__SYSTEM_UP_TIME` globals (`allow_system_uptime_global`),
/// which are an IronPLC/RuSTy runtime convention rather than a CODESYS
/// feature, and it *does* enable `allow_long_time_types` (CODESYS supports
/// the LTIME/LDATE/LTOD/LDT keywords, whereas RuSTy keeps them as
/// identifiers for OSCAT). Listed explicitly so each divergence is asserted
/// rather than assumed.
#[test]
fn codesys_dialect_enables_exactly_these_flags() {
    assert_enabled_flags(
        Dialect::Codesys,
        &[
            "allow_c_style_comments",
            "allow_missing_semicolon",
            "allow_top_level_var_global",
            "allow_constant_type_params",
            "allow_empty_var_blocks",
            "allow_time_as_function_name",
            "allow_long_time_types",
            "allow_ref_to",
            "allow_reference_to",
            "allow_pointer_to",
            "allow_adr",
            "allow_persistent_var",
            "allow_ref_arithmetic",
            "allow_ref_stack_variables",
            "allow_ref_type_punning",
            "allow_int_to_bool_initializer",
            "allow_sizeof",
            "allow_cross_family_widening",
            "allow_cross_family_conversion",
            "allow_int_literal_to_bit_string",
            "allow_partial_access_syntax",
            "allow_pragmas",
            "allow_short_circuit_operators",
            "allow_mixed_located_var_declarations",
            "allow_constant_initializer_expressions",
            "allow_bit_string_case_labels",
            "allow_paren_string_length",
            "allow_struct_initializer_expressions",
            "allow_fb_inheritance",
            "allow_enum_explicit_values",
            "allow_enum_base_type",
        ],
    );
}

/// The TwinCAT dialect is close to CODESYS (TwinCAT 3 runs on the CODESYS
/// V3 runtime) but does *not* enable the `REF_TO` reference extensions.
/// TwinCAT spells references `REFERENCE TO` (not the CODESYS `REF_TO` /
/// `REF()` / `NULL`), so it enables `allow_reference_to` and
/// `allow_pointer_to` instead, and none of `allow_ref_to`,
/// `allow_ref_arithmetic`, `allow_ref_stack_variables`, or
/// `allow_ref_type_punning` are enabled -- enabling those would accept
/// `REF_TO` code that TwinCAT itself rejects. It does enable
/// `allow_long_time_types`, since TwinCAT supports the
/// LTIME/LDATE/LTOD/LDT keywords. Listed explicitly so an accidental
/// divergence from the intended set is caught.
#[test]
fn twincat_dialect_enables_exactly_these_flags() {
    assert_enabled_flags(
        Dialect::TwinCat,
        &[
            "allow_c_style_comments",
            "allow_missing_semicolon",
            "allow_top_level_var_global",
            "allow_constant_type_params",
            "allow_empty_var_blocks",
            "allow_time_as_function_name",
            "allow_long_time_types",
            "allow_reference_to",
            "allow_pointer_to",
            "allow_adr",
            "allow_persistent_var",
            "allow_int_to_bool_initializer",
            "allow_sizeof",
            "allow_cross_family_widening",
            "allow_cross_family_conversion",
            "allow_int_literal_to_bit_string",
            "allow_partial_access_syntax",
            "allow_pragmas",
            "allow_short_circuit_operators",
            "allow_mixed_located_var_declarations",
            "allow_constant_initializer_expressions",
            "allow_bit_string_case_labels",
            "allow_paren_string_length",
            "allow_struct_initializer_expressions",
            "allow_fb_inheritance",
            "allow_enum_explicit_values",
            "allow_enum_base_type",
        ],
    );
}

/// REQ-PAB-parser-051: The `rusty` dialect preset enables partial-access syntax.
#[spec_test(REQ_PAB_parser_051)]
fn options_spec_req_pab_051_rusty_dialect_enables_partial_access_syntax() {
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    assert!(options.allow_partial_access_syntax);
}

/// REQ-PAB-parser-052: The `iec61131-3-ed3` dialect preset enables partial-access syntax.
#[spec_test(REQ_PAB_parser_052)]
fn options_spec_req_pab_052_ed3_dialect_enables_partial_access_syntax() {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assert!(options.allow_partial_access_syntax);
}

/// REQ-PAB-parser-141: The `codesys` and `twincat` dialect presets enable
/// partial-access syntax.
#[spec_test(REQ_PAB_parser_141)]
#[rstest]
#[case::codesys(Dialect::Codesys)]
#[case::twincat(Dialect::TwinCat)]
fn options_spec_req_pab_141_vendor_dialects_enable_partial_access_syntax(#[case] dialect: Dialect) {
    let options = CompilerOptions::from_dialect(dialect);
    assert!(options.allow_partial_access_syntax);
}

/// Assert that a dialect selects exactly the given policy alternatives,
/// keyed by `option_key`. Like `assert_enabled_flags`, this is the guard
/// against a preset silently changing: every policy is listed for every
/// dialect, so a new policy forces a matching update here.
fn assert_selected_policies(dialect: Dialect, expected: &[(&str, &str)]) {
    let options = CompilerOptions::from_dialect(dialect);
    let mut selected: Vec<(&str, &str)> = CompilerOptions::POLICY_DESCRIPTORS
        .iter()
        .map(|p| {
            (
                p.option_key,
                options.get_policy_by_key(p.option_key).unwrap(),
            )
        })
        .collect();
    selected.sort_unstable();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(
        selected, expected_sorted,
        "dialect {dialect} does not select exactly the expected policy alternatives"
    );
}

/// The strict dialects select every policy's default: reject anything
/// that is not a whole literal, and trap on failure (ADR-0049 rule 4).
#[rstest]
#[case::ed2(Dialect::Iec61131_3Ed2)]
#[case::ed3(Dialect::Iec61131_3Ed3)]
fn strict_dialects_select_default_policies(#[case] dialect: Dialect) {
    assert_selected_policies(
        dialect,
        &[
            ("policy_string_to_num_non_numeric", "reject"),
            ("policy_string_to_num_failure", "trap"),
        ],
    );
}

/// RuSTy rejects a string with trailing characters, but its documented
/// "never fault" contract makes the failure result 0 rather than a trap.
#[test]
fn rusty_dialect_selects_reject_and_zero() {
    assert_selected_policies(
        Dialect::Rusty,
        &[
            ("policy_string_to_num_non_numeric", "reject"),
            ("policy_string_to_num_failure", "zero"),
        ],
    );
}

/// CODESYS and TwinCAT (observed) stop parsing at the first invalid
/// character and document 0 as the result for a string that is not valid
/// in the target type.
#[rstest]
#[case::codesys(Dialect::Codesys)]
#[case::twincat(Dialect::TwinCat)]
fn codesys_family_dialects_select_ignore_trailing_and_zero(#[case] dialect: Dialect) {
    assert_selected_policies(
        dialect,
        &[
            ("policy_string_to_num_non_numeric", "ignore-trailing"),
            ("policy_string_to_num_failure", "zero"),
        ],
    );
}

#[test]
fn set_policy_by_key_when_known_key_and_alternative_then_selected() {
    let mut options = CompilerOptions::default();
    assert_eq!(
        options.set_policy_by_key("policy_string_to_num_failure", "zero"),
        Ok(())
    );
    assert_eq!(
        options.policy_string_to_num_failure,
        StringToNumFailure::Zero
    );
    assert_eq!(
        options.get_policy_by_key("policy_string_to_num_failure"),
        Some("zero")
    );
}

#[test]
fn set_policy_by_key_when_unknown_alternative_then_error_and_unchanged() {
    let mut options = CompilerOptions::default();
    assert_eq!(
        options.set_policy_by_key("policy_string_to_num_non_numeric", "wrap"),
        Err(SetPolicyError::UnknownAlternative)
    );
    assert_eq!(
        options.policy_string_to_num_non_numeric,
        StringToNumNonNumeric::Reject
    );
}

#[test]
fn set_policy_by_key_when_unknown_key_then_error() {
    let mut options = CompilerOptions::default();
    assert_eq!(
        options.set_policy_by_key("allow_c_style_comments", "reject"),
        Err(SetPolicyError::UnknownKey)
    );
    assert_eq!(options.get_policy_by_key("allow_c_style_comments"), None);
}

#[test]
fn set_policy_by_key_when_each_descriptor_alternative_then_round_trips() {
    let mut options = CompilerOptions::default();
    for p in CompilerOptions::POLICY_DESCRIPTORS {
        for alt in p.alternatives {
            assert_eq!(options.set_policy_by_key(p.option_key, alt), Ok(()));
            assert_eq!(options.get_policy_by_key(p.option_key), Some(*alt));
        }
    }
}

#[test]
fn policy_descriptors_when_called_then_default_is_first_alternative_and_keys_start_with_policy() {
    assert!(!CompilerOptions::POLICY_DESCRIPTORS.is_empty());
    let defaults = CompilerOptions::default();
    for p in CompilerOptions::POLICY_DESCRIPTORS {
        assert!(
            p.option_key.starts_with("policy_"),
            "option_key {} does not start with policy_",
            p.option_key
        );
        assert!(p.cli_flag.starts_with("--policy-"));
        assert_eq!(p.default, p.alternatives[0]);
        assert_eq!(defaults.get_policy_by_key(p.option_key), Some(p.default));
        assert!(!p.description.is_empty());
    }
}

#[test]
fn describe_dialects_when_called_then_lists_policy_selections() {
    let output = describe_dialects();
    assert!(output.contains("Behavior policies selected by \"codesys\":"));
    assert!(output.contains("--policy-string-to-num-non-numeric"));
    assert!(output.contains("ignore-trailing"));
}

#[test]
fn from_dialect_when_default_then_ed2() {
    let options = CompilerOptions::from_dialect(Dialect::default());

    assert!(!options.allow_long_time_types);
    assert!(!options.allow_ref_to);
}

#[test]
fn feature_descriptors_when_called_then_non_empty_and_stably_ordered() {
    assert!(!CompilerOptions::FEATURE_DESCRIPTORS.is_empty());
    assert_eq!(
        CompilerOptions::FEATURE_DESCRIPTORS[0].cli_flag,
        "--allow-c-style-comments"
    );
}

#[test]
fn describe_dialects_when_called_then_contains_all_dialects() {
    let output = describe_dialects();
    assert!(output.contains("iec61131-3-ed2"));
    assert!(output.contains("iec61131-3-ed3"));
    assert!(output.contains("rusty"));
    assert!(output.contains("codesys"));
    assert!(output.contains("twincat"));
}

#[test]
fn describe_dialects_when_called_then_contains_feature_flags() {
    let output = describe_dialects();
    assert!(output.contains("--allow-c-style-comments"));
    assert!(output.contains("--allow-ref-to"));
}

#[test]
fn dialect_display_when_ed2_then_cli_name() {
    assert_eq!(format!("{}", Dialect::Iec61131_3Ed2), "iec61131-3-ed2");
}

#[test]
fn dialect_display_when_rusty_then_cli_name() {
    assert_eq!(format!("{}", Dialect::Rusty), "rusty");
}

#[test]
fn dialect_display_when_codesys_then_cli_name() {
    assert_eq!(format!("{}", Dialect::Codesys), "codesys");
}

#[test]
fn dialect_display_when_twincat_then_cli_name() {
    assert_eq!(format!("{}", Dialect::TwinCat), "twincat");
}

#[test]
fn dialect_from_str_when_known_name_then_returns_variant() {
    assert_eq!("iec61131-3-ed2".parse(), Ok(Dialect::Iec61131_3Ed2));
    assert_eq!("iec61131-3-ed3".parse(), Ok(Dialect::Iec61131_3Ed3));
    assert_eq!("rusty".parse(), Ok(Dialect::Rusty));
    assert_eq!("codesys".parse(), Ok(Dialect::Codesys));
    assert_eq!("twincat".parse(), Ok(Dialect::TwinCat));
}

#[test]
fn dialect_from_str_when_unknown_name_then_returns_err() {
    let result: Result<Dialect, _> = "nonsense".parse();
    assert!(result.is_err());
}

#[test]
fn dialect_from_str_when_round_trip_then_equal() {
    for dialect in Dialect::ALL {
        assert_eq!(dialect.to_string().parse::<Dialect>(), Ok(*dialect));
    }
}

#[test]
fn feature_descriptors_when_called_then_option_key_matches_field_name() {
    let fd = &CompilerOptions::FEATURE_DESCRIPTORS[0];
    assert_eq!(fd.option_key, "allow_c_style_comments");
}

#[test]
fn feature_descriptors_when_called_then_all_option_keys_start_with_allow() {
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        assert!(
            fd.option_key.starts_with("allow_"),
            "option_key {} does not start with allow_",
            fd.option_key
        );
    }
}

#[test]
fn dialect_display_name_when_ed2_then_human_readable() {
    assert_eq!(Dialect::Iec61131_3Ed2.display_name(), "IEC 61131-3 Ed. 2");
}

#[test]
fn dialect_description_when_ed2_then_contains_edition_2() {
    assert!(Dialect::Iec61131_3Ed2.description().contains("Edition 2"));
}
