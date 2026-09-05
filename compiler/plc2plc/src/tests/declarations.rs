//! Late-bound, empty VAR, VAR_TEMP and sized-array declarations.

use super::common::*;

#[test]
fn write_to_string_late_bound_declaration() {
    use ironplc_dsl::common::{
        DataTypeDeclarationKind, LateBoundDeclaration, Library, LibraryElementKind, TypeName,
    };

    // The base type must be a user-defined name: an elementary base resolves
    // to a simple type declaration and never reaches this node. Building the
    // library directly is the only way to reach it from one source file.
    let late_bound_decl = LateBoundDeclaration {
        data_type_name: TypeName::from("MY_ALIAS"),
        base_type_name: TypeName::from("MY_BASE"),
    };

    let library = Library {
        elements: vec![LibraryElementKind::DataTypeDeclaration(
            DataTypeDeclarationKind::LateBound(late_bound_decl),
        )],
    };

    let rendered = assert_library_renders_to_parseable_text(&library, &CompilerOptions::default());

    let expected = "TYPE\n   MY_ALIAS : MY_BASE ;\nEND_TYPE\n";
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_empty_var_block() {
    let options = CompilerOptions {
        allow_empty_var_blocks: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to(
        "empty_var_block.st",
        "empty_var_block_rendered.st",
        &options,
    );
}

#[test]
fn write_to_string_var_temp() {
    assert_resource_renders_to(
        "var_temp.st",
        "var_temp_rendered.st",
        &CompilerOptions::default(),
    );
}

#[test]
fn write_to_string_when_array_of_string_with_size_then_renders_size() {
    assert_resource_renders_to(
        "array_of_string.st",
        "array_of_string_rendered.st",
        &CompilerOptions::default(),
    );
}

#[test]
fn write_to_string_when_string_parenthesis_length_then_normalizes_to_brackets() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    hostName : STRING(255);
END_VAR
END_FUNCTION_BLOCK
";
    // The parenthesis length form is an extension, so it only parses
    // with allow_paren_string_length enabled.
    let paren_options = CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    };
    let rendered = assert_round_trips(source, &paren_options);

    // The renderer always normalizes to the bracket form -- there's no
    // bracket/paren marker stored in the DSL, matching how
    // StringSpecification/StringInitializer already only store
    // length: Option<IntegerRef> with no delimiter distinction.
    assert!(rendered.contains("STRING [ 255 ]"));

    // The normalized output parses under any dialect, including the strict
    // default the source itself could not use.
    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .expect("normalized bracket form must parse without allow_paren_string_length");
}

#[test]
fn write_to_string_when_array_of_string_parenthesis_length_then_normalizes_to_brackets() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    names : ARRAY[1..10] OF STRING(255);
END_VAR
END_FUNCTION_BLOCK
";
    let paren_options = CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    };
    let rendered = assert_round_trips(source, &paren_options);

    // Same normalization as the scalar case: the DSL keeps no delimiter
    // marker, so the element type always renders with brackets.
    assert!(rendered.contains("STRING [ 255 ]"));

    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .expect("normalized bracket form must parse without allow_paren_string_length");
}

// ---------------------------------------------------------------------
// CODESYS/TwinCAT FB-instance call-style initializer (distinct node).
// ---------------------------------------------------------------------

#[test]
fn write_to_string_when_fb_instance_call_style_init_then_round_trips() {
    assert_round_trips(
        "
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
",
        &CompilerOptions::default(),
    );
}
