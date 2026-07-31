//! Comment handling and top-level parse errors.

use super::common::*;

#[test]
fn parse_program_when_has_comment_then_ok() {
    let source = "
        TYPE
        (* A comment *)
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

    let res = parse_text(source);
    assert_eq!(1, res.elements.len());
}

#[test]
fn parse_program_when_back_to_back_comments_then_ok() {
    let program = "
        TYPE
        (* A comment *)(* A comment *)
           CUSTOM_STRUCT : STRUCT 
             NAME: BOOL;
           END_STRUCT;
        END_TYPE";

    parse_text(program);
}

#[test]
fn parse_program_when_right_parent_in_comment_then_ok() {
    let program = "
        TYPE
        (* A comment) *)(* A comment *)
           CUSTOM_STRUCT : STRUCT 
             NAME: BOOL;
           END_STRUCT;
        END_TYPE";

    parse_text(program);
}

#[test]
fn parse_program_when_comment_not_closed_then_err() {
    let program = "
        TYPE
        (* A comment
            CUSTOM_STRUCT : STRUCT
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

    let res = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(res.is_err());
}

#[test]
fn parse_program_when_bad_name_then_err() {
    let program = "
        TYPE
            CUSTOM_STRUCT : STRUCT& 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

    let res = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(res.is_err());

    let err = res.unwrap_err();
    assert_eq!("Syntax error".to_owned(), err.description());
    assert_eq!("Expected ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | '\\n' (new line) | '{ ... }' (pragma) | (identifier). Found text '&' that matched token 'AND' | '&'".to_owned(), err.primary.message);
}

#[test]
fn parse_program_when_not_valid_top_item_then_err() {
    let program = "ACTION
        END_ACTION";

    let res = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(res.is_err());

    let err = res.unwrap_err();
    assert_eq!("Syntax error".to_owned(), err.description());
    assert_eq!("Expected ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | 'CONFIGURATION' | 'FUNCTION' | 'FUNCTION_BLOCK' | 'PROGRAM' | 'TYPE' | 'VAR_GLOBAL' | '\\n' (new line) | '{ ... }' (pragma). Found text 'ACTION' that matched token 'ACTION'".to_owned(), err.primary.message);
}
