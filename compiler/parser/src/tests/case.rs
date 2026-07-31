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
