//! Every symbolic variable records the source text it was written as.
//!
//! A multi-element variable (`s.x`, `a.3`, `s.inner.x`) has no span of its
//! own: `Located` joins the span of the variable it accesses with the span of
//! the element it selects. A diagnostic about the variable underlines that
//! joined span, so it has to end where the last element ends. See
//! https://github.com/ironplc/ironplc/issues/1976.

use super::common::*;
use dsl::core::Located;

/// Parses `FUNCTION_BLOCK fb ... <variable> := <variable>;` and returns the
/// source with the assignment, so a test can slice the source with the span
/// it is checking.
fn parse_variable_assignment(variable: &str) -> (String, Library) {
    let source = format!(
        "TYPE
    Inner : STRUCT y : BOOL; END_STRUCT;
    S : STRUCT x : BOOL; inner : Inner; END_STRUCT;
END_TYPE
FUNCTION_BLOCK fb
VAR
    a : INT;
    s : S;
    t : TON;
END_VAR
{variable} := {variable};
END_FUNCTION_BLOCK"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(library.is_ok(), "Parse failed: {:?}", library.err());
    (source, library.unwrap())
}

fn extract_assignment_target(library: &Library) -> Variable {
    let element = library
        .elements
        .iter()
        .find(|e| matches!(e, LibraryElementKind::FunctionBlockDeclaration(_)))
        .unwrap();
    let fb = cast!(element, LibraryElementKind::FunctionBlockDeclaration);
    let stmts = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    let assignment = cast!(&stmts.body[0], StmtKind::Assignment);
    assignment.target.clone()
}

#[rstest]
#[case::structured("s.x")]
#[case::chained_structured("s.inner.y")]
#[case::function_block_member("t.Q")]
#[case::bit_access("a.3")]
fn parse_when_multi_element_variable_is_value_then_span_covers_the_variable(
    #[case] variable: &str,
) {
    let (source, library) = parse_variable_assignment(variable);
    let value = extract_assignment_value(&library);

    let span = value.span();
    assert_eq!(
        variable,
        &source[span.start..span.end],
        "span of {value:?} should cover the variable as written"
    );
}

#[rstest]
#[case::structured("s.x")]
#[case::chained_structured("s.inner.y")]
#[case::function_block_member("t.Q")]
#[case::bit_access("a.3")]
fn parse_when_multi_element_variable_is_target_then_span_covers_the_variable(
    #[case] variable: &str,
) {
    let (source, library) = parse_variable_assignment(variable);
    let target = extract_assignment_target(&library);

    let span = target.span();
    assert_eq!(
        variable,
        &source[span.start..span.end],
        "span of {target:?} should cover the variable as written"
    );
}
