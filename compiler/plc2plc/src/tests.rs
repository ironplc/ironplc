//! Tests of renderer.
#[cfg(test)]
mod test {
    use std::fs;
    use std::path::PathBuf;

    use dsl::core::FileId;
    use proptest::prelude::*;
    use ironplc_dsl::core::Id;

    use ironplc_parser::options::ParseOptions;
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

    // Property-based tests for code generation features

    proptest! {
        // Feature: ironplc-extended-syntax, Property 6: Reference parameter modification round-trip
        #[test]
        fn property_reference_parameter_modification_round_trip(
            param_name in "[a-zA-Z][a-zA-Z0-9_]*",
            type_name in "[A-Z][A-Z0-9_]*"
        ) {
            use ironplc_dsl::common::*;
            // use ironplc_dsl::textual::*; // Unused import

            // Create a function with a reference parameter
            let ref_param = VarDecl {
                identifier: VariableIdentifier::new_symbol(&param_name),
                var_type: VariableType::Input,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::simple_uninitialized(TypeName::from(&type_name)),
                reference_annotation: Some(ReferenceAnnotation::Reference),
            };

            let function_decl = FunctionDeclaration {
                name: Id::from("test_func"),
                return_type: TypeName::from("BOOL"),
                variables: vec![ref_param],
                edge_variables: vec![],
                body: vec![],
                external_annotation: None,
            };

            let library = Library {
                elements: vec![LibraryElementKind::FunctionDeclaration(function_decl)],
            };

            // Render to string
            let rendered = write_to_string(&library).unwrap();
            
            // The rendered output should contain the reference annotation
            let ref_annotation = "{ref}";
            prop_assert!(rendered.contains(ref_annotation));
            prop_assert!(rendered.contains(&param_name));
            prop_assert!(rendered.contains(&type_name));
        }

        // Feature: ironplc-extended-syntax, Property 22: Dereference round-trip consistency
        #[test]
        fn property_dereference_round_trip_consistency(
            var_name in "[a-zA-Z][a-zA-Z0-9_]*"
        ) {
            use ironplc_dsl::common::*;
            use ironplc_dsl::textual::*;

            // Create a dereference variable
            let named_var = SymbolicVariableKind::Named(NamedVariable {
                name: Id::from(&var_name),
            });

            let deref_var = SymbolicVariableKind::Dereference(DereferenceVariable {
                referenced_variable: Box::new(named_var),
            });

            // Create an assignment using the dereference
            let assignment = Assignment {
                target: Variable::Symbolic(deref_var),
                value: ExprKind::integer_literal("42"),
            };

            let program_decl = ProgramDeclaration {
                name: Id::from("test_prog"),
                variables: vec![],
                access_variables: vec![],
                body: FunctionBlockBodyKind::stmts(vec![StmtKind::Assignment(assignment)]),
                actions: None,
            };

            let library = Library {
                elements: vec![LibraryElementKind::ProgramDeclaration(program_decl)],
            };

            // Render to string
            let rendered = write_to_string(&library).unwrap();
            
            // The rendered output should contain the dereference operator
            let expected_deref = format!("{var_name}^");
            prop_assert!(rendered.contains(&expected_deref));
        }

        // Feature: ironplc-extended-syntax, Property 26: Array element assignment round-trip
        #[test]
        fn property_array_element_assignment_round_trip(
            array_name in "[a-zA-Z][a-zA-Z0-9_]*",
            index_value in 0..100i32
        ) {
            use ironplc_dsl::common::*;
            use ironplc_dsl::textual::*;

            // Create an array variable access
            let named_var = SymbolicVariableKind::Named(NamedVariable {
                name: Id::from(&array_name),
            });

            let array_var = SymbolicVariableKind::Array(ArrayVariable {
                subscripted_variable: Box::new(named_var),
                subscripts: vec![ExprKind::integer_literal(&index_value.to_string())],
            });

            // Create an assignment to the array element
            let assignment = Assignment {
                target: Variable::Symbolic(array_var),
                value: ExprKind::integer_literal("100"),
            };

            let program_decl = ProgramDeclaration {
                name: Id::from("test_prog"),
                variables: vec![],
                access_variables: vec![],
                body: FunctionBlockBodyKind::stmts(vec![StmtKind::Assignment(assignment)]),
                actions: None,
            };

            let library = Library {
                elements: vec![LibraryElementKind::ProgramDeclaration(program_decl)],
            };

            // Render to string
            let rendered = write_to_string(&library).unwrap();
            
            // The rendered output should contain the array access syntax (with spaces)
            let expected_array = format!("{array_name} [ {index_value} ]");
            prop_assert!(rendered.contains(&expected_array));
        }

        // Feature: ironplc-extended-syntax, Property 30: Struct member access round-trip
        #[test]
        fn property_struct_member_access_round_trip(
            struct_name in "[a-zA-Z][a-zA-Z0-9_]*",
            field_name in "[a-zA-Z][a-zA-Z0-9_]*"
        ) {
            use ironplc_dsl::common::*;
            use ironplc_dsl::textual::*;

            // Create a structured variable access
            let named_var = SymbolicVariableKind::Named(NamedVariable {
                name: Id::from(&struct_name),
            });

            let struct_var = SymbolicVariableKind::Structured(StructuredVariable {
                record: Box::new(named_var),
                field: Id::from(&field_name),
            });

            // Create an assignment to the struct member
            let assignment = Assignment {
                target: Variable::Symbolic(struct_var),
                value: ExprKind::integer_literal("42"),
            };

            let program_decl = ProgramDeclaration {
                name: Id::from("test_prog"),
                variables: vec![],
                access_variables: vec![],
                body: FunctionBlockBodyKind::stmts(vec![StmtKind::Assignment(assignment)]),
                actions: None,
            };

            let library = Library {
                elements: vec![LibraryElementKind::ProgramDeclaration(program_decl)],
            };

            // Render to string
            let rendered = write_to_string(&library).unwrap();
            
            // The rendered output should contain the struct member access syntax
            let expected_struct = format!("{struct_name}.{field_name}");
            prop_assert!(rendered.contains(&expected_struct));
        }
    }
}
