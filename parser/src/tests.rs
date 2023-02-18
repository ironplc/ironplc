#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::common::Library;

    use crate::error::ParserDiagnostic;
    use crate::parse_program;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path).expect("Unable to read file")
    }

    pub fn parse_resource(name: &'static str) -> Result<Library, ParserDiagnostic> {
        let source = read_resource(name);
        parse_program(&source)
    }

    #[test]
    fn parse_variable_declarations() {
        let res = parse_resource("var_decl.st");
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
}
