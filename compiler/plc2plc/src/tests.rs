//! Tests of renderer.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::common::Library;
    use dsl::core::FileId;

    use ironplc_parser::parse_program;

    use crate::write_to_string;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // TODO move these resources to a common directory so that they can be used
        // by more than one set of tests without crossing module boundaries
        path.push("../resources/test");
        path.push(name);

        fs::read_to_string(path.clone()).expect(format!("Unable to read file {:?}", path).as_str())
    }

    pub fn parse_resource(name: &'static str) -> Library {
        let source = read_resource(name);
        parse_program(&source, &FileId::default()).unwrap()
    }

    #[test]
    fn write_to_string_arrays() {
        let res = parse_resource("array.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_conditional() {
        let res = parse_resource("conditional.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_expressions() {
        let res = parse_resource("expressions.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_nested() {
        let res = parse_resource("nested.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_strings() {
        let res = parse_resource("strings.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_textual() {
        let res = parse_resource("textual.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }

    #[test]
    fn write_to_string_type_decl() {
        let res = parse_resource("type_decl.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is stIill a work in progress
    }

    #[test]
    fn write_to_string_var_decl() {
        let res = parse_resource("var_decl.st");
        let res = write_to_string(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }
}
