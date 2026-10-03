//! OOP extension: PROPERTY declarations with GET/SET accessors, round-trip.
//!
//! A SET accessor carries an implicit input for the assigned value (see
//! `PropertyDeclaration`). The re-parse catches a renderer that writes it
//! out: the rendered VAR_INPUT would parse as a second input, and the ASTs
//! would differ.

use super::common::*;
use dsl::common::{Library, LibraryElementKind, MethodDeclaration, VarDecl};
use ironplc_analyzer::stages::analyze;
use rstest::rstest;

#[rstest]
#[case::get_and_set(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
SET
    _speed := Speed;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::get_only_with_var_block(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
VAR
    tmp : REAL;
END_VAR
    tmp := _speed;
    Speed := tmp;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::set_only_string(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _name : STRING[20];
END_VAR
PROPERTY Name : STRING[20]
SET
    _name := Name;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::set_with_var_input(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
SET
VAR_INPUT
    scale : REAL;
END_VAR
    _speed := Speed * scale;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::interleaved_with_methods(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
METHOD Start
    _speed := 1.0;
END_METHOD
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
END_PROPERTY
METHOD Stop
    _speed := 0.0;
END_METHOD
END_FUNCTION_BLOCK
"
)]
fn write_to_string_when_property_then_round_trips(#[case] source: &str) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_round_trips(source, &options);
}

/// Issue #1957: after the full analyzer pipeline, the SET accessor's
/// implicit input stays out of the rendering and its own `VAR_INPUT` stays
/// in, so parsing the rendering gives back the same accessor variables.
#[test]
fn write_to_string_when_analyzed_set_declares_var_input_then_renders_only_declared() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
SET
VAR_INPUT
    scale : REAL;
END_VAR
    _speed := Speed * scale;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(!context.has_diagnostics(), "{:?}", context.diagnostics());

    let rendered = write_to_string(&library).unwrap();
    let reparsed = parse_program(&rendered, &FileId::default(), &options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));

    let set = only_set_accessor(&reparsed);
    assert_eq!(
        names(&set.variables),
        vec!["scale"],
        "Rendered:\n{rendered}"
    );
    assert_eq!(
        names(&set.implicit_variables),
        vec!["Speed"],
        "Rendered:\n{rendered}"
    );
}

fn only_set_accessor(library: &Library) -> &MethodDeclaration {
    let fb = library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::FunctionBlockDeclaration(fb) => Some(fb),
            _ => None,
        })
        .unwrap();
    fb.properties[0].set.as_ref().unwrap()
}

fn names(variables: &[VarDecl]) -> Vec<String> {
    variables
        .iter()
        .map(|v| v.identifier.symbolic_id().unwrap().to_string())
        .collect()
}
