//! Tests of parser.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::common::Library;
    use dsl::core::FileId;
    use dsl::diagnostic::Diagnostic;

    use crate::parse_program;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../resources/test");
        path.push(name);

        fs::read_to_string(path.clone()).expect(format!("Unable to read file {:?}", path).as_str())
    }

    pub fn parse_resource(name: &'static str) -> Result<Library, Diagnostic> {
        let source = read_resource(name);
        parse_program(&source, &FileId::default())
    }

    #[test]
    fn parse_variable_declarations() {
        let res = parse_resource("var_decl.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_strings() {
        let res = parse_resource("strings.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_type_decl() {
        let res = parse_resource("type_decl.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_textual() {
        let res = parse_resource("textual.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_conditional() {
        let res = parse_resource("conditional.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_oscat() {
        // OSCAT files have a header that as far as I can tell is not valid
        // but it is common.
        let res = parse_resource("oscat.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_expressions() {
        let res = parse_resource("expressions.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_array() {
        let res = parse_resource("array.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_nested() {
        let res: Result<Library, Diagnostic> = parse_resource("nested.st");
        assert!(res.is_ok())
    }

    #[test]
    fn parse_program_when_has_comment_then_ok() {
        let source = "
        TYPE
        (* A comment *)
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let res = parse_program(source, &FileId::default()).unwrap();
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

        let res = parse_program(program, &FileId::default());
        assert!(res.is_ok());
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

        let res = parse_program(program, &FileId::default());
        assert!(res.is_ok());
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

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!(
            "Unmatched token in source text".to_owned(),
            err.description()
        );
        assert_eq!("The text '(* A comment\n            CUSTOM_STRUCT : STRUCT \n                NAME: BOOL;\n            END_STRUCT;\n        END_TYPE' is not valid IEC 61131-3 text at line 3 colum 9.".to_owned(), err.primary.message);
    }

    #[test]
    fn parse_program_when_bad_name_then_err() {
        let program = "
        TYPE
            CUSTOM_STRUCT : STRUCT& 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!("Syntax error".to_owned(), err.description());
        assert_eq!("Expected  | ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | '\\n' (new line). Found text '&' that matched token 'AND' | '&'".to_owned(), err.primary.message);
    }

    #[test]
    fn parse_program_when_not_valid_top_item_then_err() {
        let program = "ACTION
        END_ACTION";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!("Syntax error".to_owned(), err.description());
        assert_eq!("Expected ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | 'CONFIGURATION' | 'FUNCTION' | 'FUNCTION_BLOCK' | 'PROGRAM' | 'TYPE' | '\\n' (new line). Found text 'ACTION' that matched token 'ACTION'".to_owned(), err.primary.message);
    }
}
