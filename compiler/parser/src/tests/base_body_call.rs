//! OOP extension: `SUPER^();`, a call of the body of the base block.
//!
//! Whitespace between its tokens is covered by the `base_body_call` row in
//! `tests/whitespace.rs`; this file asserts the AST shape and what is not
//! accepted.

use super::common::*;
use rstest::rstest;

/// The source of a function block that extends `FB_Base`, with `body` as
/// its body and `method` as the body of its one method `Run`.
fn derived_block(body: &str, method: &str) -> String {
    format!(
        "FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
{body}
METHOD Run
{method}
END_METHOD
END_FUNCTION_BLOCK"
    )
}

/// The first statement of `statements`, as a call of the base block's body.
fn base_body_call(statements: &[StmtKind]) -> &BaseBodyCall {
    cast!(&statements[0], StmtKind::BaseBodyCall)
}

#[test]
fn parse_when_super_caret_call_in_body_then_base_body_call() {
    let source = derived_block("    SUPER^();", "    ;");
    let library = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let body = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    let call = base_body_call(&body.body);
    assert_eq!(call.base.kind, SelfRefKind::Super);
}

#[test]
fn parse_when_super_caret_call_in_method_then_base_body_call() {
    let source = derived_block("", "    SUPER^();");
    let library = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let call = base_body_call(&extract_fb(&library).methods[0].body);
    assert_eq!(call.base.kind, SelfRefKind::Super);
}

/// Only `SUPER^()` is a call of a body. `THIS^()` would call the block's
/// own body, and no target is known to pass an argument to `SUPER^()`, so
/// neither is accepted (ADR-0036). The caret is mandatory, as it is for
/// `SUPER^.member`.
#[rstest]
#[case::this_call("    THIS^();")]
#[case::named_argument("    SUPER^(x := 1);")]
#[case::positional_argument("    SUPER^(1);")]
#[case::without_caret("    SUPER();")]
fn parse_when_not_an_empty_super_caret_call_then_err(#[case] body: &str) {
    let source = derived_block(body, "    ;");
    let result = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance());
    assert!(result.is_err());
}

/// Without `allow_fb_inheritance`, `SUPER` is an ordinary identifier, so
/// `SUPER^();` stays the syntax error it always was.
#[test]
fn parse_when_standard_mode_then_super_caret_call_is_err() {
    let source = "FUNCTION_BLOCK FB
VAR
    SUPER : INT;
END_VAR
    SUPER^();
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
