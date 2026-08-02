//! CASE statement label parsing.

use super::common::*;

#[test]
fn parse_when_case_branch_empty_and_followed_by_another_label_then_ok() {
    // An empty CASE branch isn't strict IEC 61131-3 (the standard only
    // allows an explicit empty statement, `5: ;`) -- this is the
    // `--allow-missing-semicolon` vendor extension filling in the
    // dropped `;`, so it must be gated behind that flag.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    1: y := 1;
    5: (* no statement here, falls through to nothing *)
    10: y := 3;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &with_missing_semicolon_flag())
        .expect("empty CASE branch followed by another label must parse");
    let case = extract_case(&library);
    assert_eq!(case.statement_groups.len(), 3);
    assert!(case.statement_groups[1].statements.is_empty());
}

#[test]
fn parse_when_case_branch_empty_and_last_before_end_case_then_ok() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    1: y := 1;
    5: (* no statement here *)
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &with_missing_semicolon_flag())
        .expect("empty CASE branch as the last one must parse");
    let case = extract_case(&library);
    assert_eq!(case.statement_groups.len(), 2);
    assert!(case.statement_groups[1].statements.is_empty());
}

#[test]
fn parse_when_case_branch_empty_and_flag_not_set_then_err() {
    // Strict IEC 61131-3 (the default dialect) must still reject this --
    // only `--allow-missing-semicolon` fills in the dropped `;`.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    1: y := 1;
    5: (* no statement here *)
END_CASE;
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_case_branch_has_statement_then_regression_ok() {
    // Regression: an ordinary populated CASE branch must be unaffected.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    1: y := 1;
    5: y := 2;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default())
        .expect("populated CASE branch must still parse");
    let case = extract_case(&library);
    assert_eq!(case.statement_groups.len(), 2);
    assert_eq!(case.statement_groups[1].statements.len(), 1);
}

// -----------------------------------------------------------------
// Hex/binary/octal bit-string literals as CASE labels. Parsing is
// dialect-agnostic: the parser always produces a
// CaseSelectionKind::BitStringLiteral for these; the
// --allow-bit-string-case-labels flag is enforced later, by the
// analyzer (see rule_case_bit_string_label). See
// specs/plans/2026-07-26-twincat-case-label-bit-string-literals.md.
// -----------------------------------------------------------------

#[test]
fn parse_when_case_label_is_hex_and_binary_literal_then_parses_as_bit_string_literal() {
    // The real motivating shape. An integer selector keeps everything but
    // the label form standard-conformant.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#D012: y := 1;
    2#1010: y := 2;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default())
        .expect("hex/binary CASE labels must parse");
    let case = extract_case(&library);
    assert_eq!(case.statement_groups.len(), 2);

    let hex_selector = &case.statement_groups[0].selectors[0];
    let hex_lit = cast!(hex_selector, CaseSelectionKind::BitStringLiteral);
    assert_eq!(hex_lit.value.value, 0xD012);

    let bin_selector = &case.statement_groups[1].selectors[0];
    let bin_lit = cast!(bin_selector, CaseSelectionKind::BitStringLiteral);
    assert_eq!(bin_lit.value.value, 0b1010);
}

#[test]
fn parse_when_case_label_is_octal_literal_then_parses_as_bit_string_literal() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    8#17: y := 1;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default())
        .expect("octal CASE label must parse");
    let case = extract_case(&library);
    let selector = &case.statement_groups[0].selectors[0];
    let lit = cast!(selector, CaseSelectionKind::BitStringLiteral);
    assert_eq!(lit.value.value, 0o17);
}

#[test]
fn parse_when_case_label_is_plain_decimal_then_regression_still_signed_integer() {
    // Regression: a plain decimal label must not be shadowed by the
    // new BitStringLiteral alternative.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    5: y := 1;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default())
        .expect("plain decimal CASE label must still parse");
    let case = extract_case(&library);
    let selector = &case.statement_groups[0].selectors[0];
    assert!(matches!(selector, CaseSelectionKind::SignedInteger(_)));
}
