//! Mixed located / non-located variable declarations.

use super::common::*;

#[test]
fn write_to_string_when_mixed_located_var_declaration_then_round_trips() {
    use dsl::common::{LibraryElementKind, VariableIdentifier};

    let source = "
FUNCTION_BLOCK FB_Example
VAR
    tempSensor AT%I*: INT;
    fbComm : BOOL;
END_VAR
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_mixed_located_var_declarations: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("AT %I*"));
    assert!(rendered.contains("tempSensor"));
    assert!(rendered.contains("fbComm"));

    // The renderer emits one VAR...END_VAR block per declaration
    // (pre-existing behavior, not specific to this feature), so the
    // located and plain variables end up in separate rendered blocks.
    // Re-parsing therefore no longer sees them as "mixed" (each
    // rendered block contains only one variable), so a strict library
    // equality check against the original isn't meaningful here.
    // Instead, verify the rendered output re-parses cleanly under the
    // same flag and both variables keep their original shape.
    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();

    let fb = match &library_rendered.elements[0] {
        LibraryElementKind::FunctionBlockDeclaration(fb) => fb,
        other => panic!("expected FunctionBlockDeclaration, got {other:?}"),
    };
    assert_eq!(fb.variables.len(), 2);
    let temp_sensor = fb
            .variables
            .iter()
            .find(|v| matches!(&v.identifier, VariableIdentifier::Direct(d) if d.name.as_ref().map(|n| n.to_string()) == Some("tempSensor".to_string())))
            .unwrap();
    assert!(matches!(
        &temp_sensor.identifier,
        VariableIdentifier::Direct(_)
    ));

    let fb_comm = fb
            .variables
            .iter()
            .find(|v| matches!(&v.identifier, VariableIdentifier::Symbol(id) if id.to_string() == "fbComm"))
            .unwrap();
    assert!(matches!(&fb_comm.identifier, VariableIdentifier::Symbol(_)));
}
