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

// ---------------------------------------------------------------------
// CODESYS/TwinCAT FB-instance call-style initializer.
// See specs/plans/2026-07-31-twincat-inline-fb-call-style-initializer.md.
// ---------------------------------------------------------------------

#[test]
fn write_to_string_when_fb_instance_call_style_init_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3, THIS);
END_VAR
END_FUNCTION_BLOCK
";
    let library_original =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("FB_Comm ( retries := 3 , THIS )"));

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
            .expect("rendered output must parse");
    assert_eq!(library_original, library_rendered);
}
