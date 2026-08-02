//! Duration and date literal spec-conformance tests.

use super::common::*;

#[rstest]
#[case::lower_t_lower("t#5s")]
#[case::upper_t_lower("T#5s")]
#[case::keyword_upper("TIME#5s")]
#[case::keyword_lower("time#5s")]
#[case::keyword_mixed("Time#5s")]
fn duration_spec_req_tl_002_prefix_case_insensitive(#[case] literal: &str) {
    // REQ-TL-002: prefix is recognized case-insensitively.
    let source = duration_program(literal);
    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "parse failed for {literal}: {:?}",
        result.err()
    );
}

#[rstest]
#[case("T#5us")]
#[case("T#5ns")]
fn duration_spec_req_tl_010_unsupported_unit_rejected(#[case] literal: &str) {
    // REQ-TL-010: units outside {d, h, m, s, ms} are parse errors.
    let source = duration_program(literal);
    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err(), "expected parse error for {literal}");
}

#[rstest]
#[case::s("T#5S", Duration::seconds(5))]
#[case::ms("T#100MS", Duration::milliseconds(100))]
#[case::h("T#1H", Duration::hours(1))]
#[case::d("T#1D", Duration::days(1))]
#[case::m("T#30M", Duration::minutes(30))]
fn duration_spec_req_tl_011_unit_suffix_uppercase_accepted(
    #[case] literal: &str,
    #[case] expected: Duration,
) {
    // REQ-TL-011: unit suffixes are case-insensitive.
    let source = duration_program(literal);
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(extract_duration(&library).interval, expected);
}

#[rstest]
#[case::ms_capital_m("T#500Ms", Duration::milliseconds(500))]
#[case::ms_capital_s("T#500mS", Duration::milliseconds(500))]
fn duration_spec_req_tl_011_unit_suffix_mixed_case_accepted(
    #[case] literal: &str,
    #[case] expected: Duration,
) {
    // REQ-TL-011: mixed-case unit suffixes parse identically to lowercase.
    let source = duration_program(literal);
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(extract_duration(&library).interval, expected);
}

#[test]
fn duration_spec_req_tl_012_ms_matched_before_m() {
    // REQ-TL-012: `T#100ms` is 100 milliseconds, not 100 minutes.
    let source = duration_program("T#100ms");
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(
        extract_duration(&library).interval,
        Duration::milliseconds(100)
    );
}

#[test]
#[ignore = "compound interval grammar is unreachable; see specs/design/time-literals.md Future Work"]
fn duration_spec_req_tl_021_compound_interval() {
    // REQ-TL-021: compound interval with parts in descending magnitude.
    let source = duration_program("T#1d2h30m15s500ms");
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let expected = Duration::days(1)
        + Duration::hours(2)
        + Duration::minutes(30)
        + Duration::seconds(15)
        + Duration::milliseconds(500);
    assert_eq!(extract_duration(&library).interval, expected);
}

#[rstest]
#[case::lower("T#1d_2h_30m_5s_100ms")]
#[case::upper("T#1D_2H_30M_5S_100MS")]
#[ignore = "compound interval grammar is unreachable; see specs/design/time-literals.md Future Work"]
fn duration_spec_req_tl_022_compound_with_underscore(#[case] literal: &str) {
    // REQ-TL-022: optional `_` separator in compound intervals.
    let source = duration_program(literal);
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let expected = Duration::days(1)
        + Duration::hours(2)
        + Duration::minutes(30)
        + Duration::seconds(5)
        + Duration::milliseconds(100);
    assert_eq!(extract_duration(&library).interval, expected);
}

#[test]
fn duration_spec_req_tl_023_negative_duration() {
    // REQ-TL-023: optional leading `-` negates the interval.
    let source = duration_program("T#-5s");
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(extract_duration(&library).interval, Duration::seconds(-5));
}

#[rstest]
#[case("D#2026-01-01")]
#[case("d#2026-01-01")]
fn date_prefix_case_insensitive(#[case] literal: &str) {
    // Sanity check for the `dt_sep("D")` simplification.
    let source = format!(
        "FUNCTION fun:DATE\nVAR\n    dv : DATE := {literal};\nEND_VAR\nfun := dv;\nEND_FUNCTION"
    );
    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "parse failed for {literal}: {:?}",
        result.err()
    );
}
