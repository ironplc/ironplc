//! Tests of renderer.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::core::FileId;

    use ironplc_parser::options::CompilerOptions;
    use ironplc_parser::parse_program;
    use ironplc_test::read_shared_resource;

    use crate::write_to_string;

    pub fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path.clone()).unwrap_or_else(|_| panic!("Unable to read file {path:?}"))
    }

    pub fn parse_and_render_resource(name: &'static str) -> String {
        let source = read_shared_resource(name);
        let library =
            parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
        write_to_string(&library).unwrap()
    }

    #[test]
    fn write_to_string_arrays() {
        let rendered = parse_and_render_resource("array.st");
        let expected = read_resource("array_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_when_array_in_function_var_then_renders() {
        let rendered = parse_and_render_resource("array_in_function_var.st");
        let expected = read_resource("array_in_function_var_rendered.st");
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

    #[test]
    fn write_to_string_sized_string_contexts() {
        let rendered = parse_and_render_resource("sized_string_contexts.st");
        let expected = read_resource("sized_string_contexts_rendered.st");
        assert_eq!(rendered, expected);
    }

    pub fn parse_and_render_resource_edition3(name: &'static str) -> String {
        let source = read_shared_resource(name);
        let options = CompilerOptions {
            allow_iec_61131_3_2013: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(&source, &FileId::default(), &options).unwrap();
        write_to_string(&library).unwrap()
    }

    fn parse_and_render_edition3(source: &str) -> String {
        let options = CompilerOptions {
            allow_iec_61131_3_2013: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(source, &FileId::default(), &options).unwrap();
        write_to_string(&library).unwrap()
    }

    #[test]
    fn write_to_string_ref() {
        let rendered = parse_and_render_resource_edition3("ref.st");
        let expected = read_resource("ref_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_when_ref_to_var_decl_then_preserves_ref_to() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
END_PROGRAM",
        );
        assert!(
            rendered.contains("REF_TO INT"),
            "Expected REF_TO INT in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_ref_to_array_var_decl_then_preserves_ref_to() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
END_PROGRAM",
        );
        assert!(
            rendered.contains("REF_TO ARRAY"),
            "Expected REF_TO ARRAY in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_ref_to_var_decl_with_null_init_then_preserves() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    x : REF_TO INT := NULL;
END_VAR
END_PROGRAM",
        );
        assert!(
            rendered.contains("REF_TO INT := NULL"),
            "Expected REF_TO INT := NULL in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_ref_to_var_decl_with_ref_init_then_preserves() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT := REF(counter);
END_VAR
END_PROGRAM",
        );
        assert!(
            rendered.contains("REF_TO INT := REF("),
            "Expected REF_TO INT := REF(...) in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_deref_assign_then_preserves_caret() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    myRef : REF_TO INT;
END_VAR
    myRef^ := 42;
END_PROGRAM",
        );
        assert!(
            rendered.contains("myRef^ :="),
            "Expected myRef^ := in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_deref_expression_then_preserves_caret() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    myRef : REF_TO INT;
    value : INT;
END_VAR
    value := myRef^;
END_PROGRAM",
        );
        assert!(
            rendered.contains("myRef ^"),
            "Expected myRef ^ in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_deref_array_expression_then_preserves() {
        let rendered = parse_and_render_edition3(
            "FUNCTION my_func : BYTE
VAR_INPUT
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
    my_func := PT^[0];
END_FUNCTION",
        );
        assert!(
            rendered.contains("PT^"),
            "Expected PT^ in output, got: {rendered}"
        );
        assert!(
            rendered.contains("[ 0 ]"),
            "Expected array subscript in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_ref_expression_then_preserves() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT;
END_VAR
    x := REF(counter);
END_PROGRAM",
        );
        assert!(
            rendered.contains("REF("),
            "Expected REF( in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_null_expression_then_preserves() {
        let rendered = parse_and_render_edition3(
            "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
    x := NULL;
END_PROGRAM",
        );
        assert!(
            rendered.contains("NULL"),
            "Expected NULL in output, got: {rendered}"
        );
    }

    #[test]
    fn write_to_string_when_ref_to_type_decl_then_preserves() {
        let rendered = parse_and_render_edition3("TYPE IntRef : REF_TO INT; END_TYPE");
        let expected = "TYPE\n   IntRef : REF_TO INT ;\nEND_TYPE\n";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_late_bound_declaration() {
        use ironplc_dsl::common::{
            DataTypeDeclarationKind, LateBoundDeclaration, Library, LibraryElementKind, TypeName,
        };

        // Create a library with a late bound declaration in code
        let late_bound_decl = LateBoundDeclaration {
            data_type_name: TypeName::from("MY_ALIAS"),
            base_type_name: TypeName::from("INT"),
        };

        let library = Library {
            elements: vec![LibraryElementKind::DataTypeDeclaration(
                DataTypeDeclarationKind::LateBound(late_bound_decl),
            )],
        };

        // Render the library to string
        let result = crate::write_to_string(&library).unwrap();

        // Expected output should be a TYPE declaration with the alias
        let expected = "TYPE\n   MY_ALIAS : INT ;\nEND_TYPE\n";
        assert_eq!(result, expected);
    }

    fn parse_and_render_resource_empty_var_blocks(name: &'static str) -> String {
        let source = read_shared_resource(name);
        let options = CompilerOptions {
            allow_empty_var_blocks: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(&source, &FileId::default(), &options).unwrap();
        write_to_string(&library).unwrap()
    }

    #[test]
    fn write_to_string_empty_var_block() {
        let rendered = parse_and_render_resource_empty_var_blocks("empty_var_block.st");
        let expected = read_resource("empty_var_block_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_var_temp() {
        let rendered = parse_and_render_resource("var_temp.st");
        let expected = read_resource("var_temp_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_when_array_of_string_with_size_then_renders_size() {
        let rendered = parse_and_render_resource("array_of_string.st");
        let expected = read_resource("array_of_string_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_when_time_function_decl_then_round_trips() {
        let source = read_shared_resource("time_function_decl.st");
        let options = CompilerOptions {
            allow_time_as_function_name: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(&source, &FileId::default(), &options).unwrap();
        let rendered = write_to_string(&library).unwrap();
        let expected = read_resource("time_function_decl_rendered.st");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn write_to_string_sizeof() {
        let source = read_shared_resource("sizeof.st");
        let options = CompilerOptions {
            allow_sizeof: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(&source, &FileId::default(), &options).unwrap();
        let rendered = write_to_string(&library).unwrap();
        let expected = read_resource("sizeof_rendered.st");
        assert_eq!(rendered, expected);
    }
}
