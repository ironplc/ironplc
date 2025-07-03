//! Tests of renderer.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::core::FileId;

    use ironplc_parser::options::ParseOptions;
    use ironplc_parser::parse_program;
    use ironplc_test::read_shared_resource;

    use crate::write_to_string;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path.clone())
            .unwrap_or_else(|_| panic!("Unable to read file {path:?}"))
    }

    pub fn parse_and_render_resource(name: &'static str) -> String {
        let source = read_shared_resource(name);
        let library = parse_program(&source, &FileId::default(), &ParseOptions::default()).unwrap();
        write_to_string(&library).unwrap()
    }

    #[test]
    fn write_to_string_arrays() {
        let rendered = parse_and_render_resource("array.st");
        let expected = read_resource("array_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_conditional() {
        let rendered = parse_and_render_resource("conditional.st");
        let expected = read_resource("conditional_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_configuration() {
        let rendered = parse_and_render_resource("configuration.st");
        let expected = read_resource("configuration_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_expressions() {
        let rendered = parse_and_render_resource("expressions.st");
        let expected = read_resource("expressions_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_inout_var_decl() {
        let rendered = parse_and_render_resource("inout_var_decl.st");
        let expected = read_resource("inout_var_decl_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_input_var_decl() {
        let rendered = parse_and_render_resource("input_var_decl.st");
        let expected = read_resource("input_var_decl_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_literal() {
        let rendered = parse_and_render_resource("literal.st");
        let expected = read_resource("literal_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_nested() {
        let rendered = parse_and_render_resource("nested.st");
        let expected = read_resource("nested_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_program() {
        let rendered = parse_and_render_resource("program.st");
        let expected = read_resource("program_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_sfc() {
        let rendered = parse_and_render_resource("sfc.st");
        let expected = read_resource("sfc_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_strings() {
        let rendered = parse_and_render_resource("strings.st");
        let expected = read_resource("strings_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_textual() {
        let rendered = parse_and_render_resource("textual.st");
        let expected = read_resource("textual_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_type_decl() {
        let rendered = parse_and_render_resource("type_decl.st");
        let expected = read_resource("type_decl_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_var_decl() {
        let rendered = parse_and_render_resource("var_decl.st");
        let expected = read_resource("var_decl_rendered.st");
        assert_eq!(rendered, expected);
    }
}
