//! Short-circuit `AND_THEN` operator parsing.

use super::common::*;

#[test]
fn parse_when_and_then_then_ok_and_compare_op_and_then() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    a : BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
result := a AND_THEN b;
END_FUNCTION_BLOCK";
    let library = parse_program(
        source,
        &FileId::default(),
        &opts_with_short_circuit_operators(),
    )
    .unwrap();
    let value = extract_assignment_value(&library);
    let compare = cast!(&value.kind, ExprKind::Compare);
    assert_eq!(compare.op, CompareOp::AndThen);
}

#[test]
fn parse_when_and_then_real_world_shape_then_ok() {
    // The real motivating shape: guarding a dereference behind a
    // null-pointer check.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    ptr : REF_TO INT;
    result : BOOL;
END_VAR
result := ptr <> 0 AND_THEN ptr^ = 99;
END_FUNCTION_BLOCK";
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

#[test]
fn parse_when_and_then_and_disabled_then_parses_as_identifiers() {
    // AND_THEN demotes to an ordinary identifier when the flag is
    // off, matching the pattern used for every other dialect-extension
    // keyword.
    let source = "
FUNCTION_BLOCK FB_ALL_AND_THEN_AS_VAR
VAR
    AND_THEN : INT;
END_VAR
AND_THEN := 1;
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "AND_THEN must remain a valid identifier in standard mode: {:?}",
        result.err()
    );
}
