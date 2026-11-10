//! Function-call expressions and bit-access variables.

use super::common::*;

#[test]
fn parse_when_mod_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := MOD(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_and_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := AND(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_or_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := OR(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_xor_function_call_then_parses() {
    parse_text(
        "FUNCTION_BLOCK CALLER
VAR
    result : BOOL;
    a : BOOL;
    b : BOOL;
END_VAR
    result := XOR(a, b);
END_FUNCTION_BLOCK",
    );
}

#[test]
fn parse_when_bit_access_then_succeeds() {
    let res = parse_resource("bit_access.st");
    assert!(res.is_ok())
}

#[test]
fn parse_when_bit_access_then_returns_bit_access_variable() {
    let lib = parse_text(
        "FUNCTION_BLOCK FB1
VAR
    x : WORD;
    y : BOOL;
END_VAR
    y := x.0;
END_FUNCTION_BLOCK",
    );
    let fb = cast!(
        &lib.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    let s = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    let assign = cast!(&s.body[0], StmtKind::Assignment);
    // The value (RHS) should be a bit access variable x.0
    let var_sym = cast!(&assign.value.kind, ExprKind::Variable);
    let var_kind = cast!(var_sym, Variable::Symbolic);
    let var = cast!(var_kind, SymbolicVariableKind::BitAccess);
    assert_eq!(var.variable.as_ref().to_string(), "x");
    assert_eq!(var.index.value, 0);
}

#[test]
fn parse_when_struct_access_with_bit_access_support_then_still_succeeds() {
    parse_text(
        "FUNCTION_BLOCK FB1
VAR
    counter : CounterST;
    result : INT;
END_VAR
    result := counter.OUT;
END_FUNCTION_BLOCK",
    );
}

fn parse_fb_call_params(arguments: &str) -> Result<Vec<ParamAssignmentKind>, String> {
    let source = format!(
        "PROGRAM main
VAR
    f : FB_X;
    v : BOOL;
    w : BOOL;
END_VAR
f({arguments});
END_PROGRAM"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default())
        .map_err(|d| format!("{d:?}"))?;
    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    let stmts = cast!(&program.body, FunctionBlockBodyKind::Statements);
    let call = cast!(&stmts.body[0], StmtKind::FbCall);
    Ok(call.params.clone())
}

#[rstest]
#[case::omitted_input_then_trailing_comma("a := v, b :=,\n", 1)]
#[case::omitted_input_between("a := v, b :=, c := w", 2)]
#[case::omitted_output("a := v, Error=> ,\n", 1)]
#[case::omitted_output_last("a := v, Error=>", 1)]
#[case::trailing_comma("a := v, ", 1)]
fn parse_when_call_argument_has_no_value_then_it_is_dropped(
    #[case] arguments: &str,
    #[case] expected: usize,
) {
    let params = parse_fb_call_params(arguments).unwrap();

    assert_eq!(expected, params.len());
}

#[rstest]
#[case::only_comma(",")]
#[case::leading_comma(", a := v")]
#[case::double_comma("a := v,, b := w")]
fn parse_when_call_argument_list_has_stray_comma_then_err(#[case] arguments: &str) {
    assert!(parse_fb_call_params(arguments).is_err());
}
