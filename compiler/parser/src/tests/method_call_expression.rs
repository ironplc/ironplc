//! OOP extension: method calls in expression position (`v := m.Get();`).
//! The statement form is in `methods.rs`; both share one grammar rule.

use super::common::*;

/// Parses `body` as the statements of a function, and returns them.
fn parse_body(body: &str) -> Vec<StmtKind> {
    let source = format!(
        "
FUNCTION F : INT
VAR
    m : FB_Motor;
    v : REAL;
END_VAR
{body}
END_FUNCTION"
    );
    let library = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let function = library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::FunctionDeclaration(f) => Some(f),
            _ => None,
        })
        .unwrap();
    function.body.clone()
}

fn assigned_value(stmt: &StmtKind) -> &Expr {
    &cast!(stmt, StmtKind::Assignment).value
}

fn method_call(expr: &Expr) -> &MethodCall {
    cast!(&expr.kind, ExprKind::MethodCall)
}

#[test]
fn parse_when_method_call_assigned_then_value_is_method_call_expression() {
    let body = parse_body("v := m.GetSpeed();");

    let call = method_call(assigned_value(&body[0]));
    assert_eq!(call.receiver, MethodReceiver::Instance(Id::from("m")));
    assert_eq!(call.method, Id::from("GetSpeed"));
    assert!(call.params.is_empty());
}

#[test]
fn parse_when_method_call_with_arguments_in_expression_then_params_kept() {
    let body = parse_body("v := m.Scaled(2.0, offset := 1.0);");

    let call = method_call(assigned_value(&body[0]));
    assert_eq!(call.params.len(), 2);
    assert!(matches!(
        call.params[0],
        ParamAssignmentKind::PositionalInput(_)
    ));
    assert!(matches!(call.params[1], ParamAssignmentKind::NamedInput(_)));
}

#[test]
fn parse_when_method_call_is_if_condition_then_condition_is_method_call() {
    let body = parse_body("IF m.IsReady() THEN v := 1.0; END_IF;");

    let if_stmt = cast!(&body[0], StmtKind::If);
    assert_eq!(method_call(&if_stmt.expr).method, Id::from("IsReady"));
}

#[test]
fn parse_when_method_call_is_binary_operand_then_operand_is_method_call() {
    let body = parse_body("v := 1.0 + m.Value();");

    let binary = cast!(&assigned_value(&body[0]).kind, ExprKind::BinaryOp);
    assert_eq!(method_call(&binary.right).method, Id::from("Value"));
}

#[test]
fn parse_when_method_call_is_argument_of_method_call_then_nested() {
    let body = parse_body("v := m.Scaled(m.Factor());");

    let outer = method_call(assigned_value(&body[0]));
    let ParamAssignmentKind::PositionalInput(arg) = &outer.params[0] else {
        panic!("expected a positional argument, got {:?}", outer.params[0]);
    };
    assert_eq!(method_call(&arg.expr).method, Id::from("Factor"));
}

#[test]
fn parse_when_self_ref_method_call_in_expression_then_receiver_is_self_ref() {
    let body = parse_body("v := THIS^.GetSpeed();");

    let call = method_call(assigned_value(&body[0]));
    assert!(matches!(call.receiver, MethodReceiver::SelfRef(_)));
}

/// Without the parentheses, `m.x` is still a field read.
#[test]
fn parse_when_structured_field_without_call_then_still_variable() {
    let body = parse_body("v := m.speed;");

    assert!(matches!(
        assigned_value(&body[0]).kind,
        ExprKind::Variable(_)
    ));
}
