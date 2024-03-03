//! Tests of renderer.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::common::Library;
    use dsl::core::FileId;

    use ironplc_parser::parse_program;

    use crate::write;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path).expect("Unable to read file")
    }

    pub fn parse_resource(name: &'static str) -> Library {
        let source = read_resource(name);
        parse_program(&source, &FileId::default()).unwrap()
    }

    #[test]
    fn parse_print_variable_declarations() {
        let res = parse_resource("var_decl.st");
        let res = write(&res).unwrap();
        assert!(!res.is_empty());
        // TODO this is still a work in progress
    }
}
