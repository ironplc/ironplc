//! Late-bound, empty VAR, VAR_TEMP and sized-array declarations.

use super::common::*;

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
