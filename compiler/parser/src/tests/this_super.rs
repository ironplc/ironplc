//! OOP extension: `THIS^` and `SUPER^` self/base references.

use super::common::*;

/// Returns the statements of the first method of the first function block.
fn first_method_body(library: &Library) -> &Vec<StmtKind> {
    &extract_fb(library).methods[0].body
}

/// Wraps `body` in a function block with a method, since `THIS^`/`SUPER^`
/// only make sense inside a method body.
fn parse_in_method(body: &str) -> Library {
    let source = format!(
        "FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    count : INT;
END_VAR
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
    );
    parse_program(&source, &FileId::default(), &opts_with_fb_inheritance()).unwrap()
}

/// Proves THIS/SUPER remain valid identifiers in standard IEC 61131-3
/// mode, mirroring the prerequisite established for the other OOP
/// keywords in `fb_inheritance.rs` and `methods.rs`.
#[test]
fn parse_when_standard_mode_then_this_super_are_valid_identifiers() {
    let program = "
FUNCTION_BLOCK FB_ALL_SELF_REF_KEYWORDS_AS_VARS
VAR
    THIS : INT;
    SUPER : INT;
END_VAR

THIS := 1;
SUPER := 2;
END_FUNCTION_BLOCK
";
    let result = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_ok());
}

/// Without the flag, `THIS^.x` keeps its pre-existing meaning: a
/// dereference of an ordinary variable named THIS. This slice must not
/// change how any program parses in standard mode.
#[test]
fn parse_when_standard_mode_then_this_caret_is_ordinary_deref() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    THIS : INT;
END_VAR
THIS^.count := 1;
END_FUNCTION_BLOCK";
    let library = parse_text(source);
    let fb = extract_fb(&library);
    let body = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    let assignment = cast!(&body.body[0], StmtKind::Assignment);
    let symbolic = cast!(&assignment.target, Variable::Symbolic);
    let structured = cast!(symbolic, SymbolicVariableKind::Structured);
    // The head is a plain named variable, NOT a self reference.
    let named = cast!(structured.record.as_ref(), SymbolicVariableKind::Deref);
    assert!(matches!(
        named.variable.as_ref(),
        SymbolicVariableKind::Named(_)
    ));
}

#[test]
fn parse_when_this_caret_field_assigned_then_self_ref_head() {
    let library = parse_in_method("    THIS^.count := 1;");
    let assignment = cast!(&first_method_body(&library)[0], StmtKind::Assignment);
    let symbolic = cast!(&assignment.target, Variable::Symbolic);
    let structured = cast!(symbolic, SymbolicVariableKind::Structured);
    assert_eq!(structured.field, Id::from("count"));
    let self_ref = cast!(structured.record.as_ref(), SymbolicVariableKind::SelfRef);
    assert_eq!(self_ref.kind, SelfRefKind::This);
}

#[test]
fn parse_when_super_caret_field_read_then_self_ref_head() {
    let library = parse_in_method("    count := SUPER^.count;");
    let assignment = cast!(&first_method_body(&library)[0], StmtKind::Assignment);
    let variable = cast!(&assignment.value.kind, ExprKind::Variable);
    let symbolic = cast!(variable, Variable::Symbolic);
    let structured = cast!(symbolic, SymbolicVariableKind::Structured);
    let self_ref = cast!(structured.record.as_ref(), SymbolicVariableKind::SelfRef);
    assert_eq!(self_ref.kind, SelfRefKind::Super);
}

/// The element chain composes with a self-reference head the same way it
/// does with a named head.
#[test]
fn parse_when_this_caret_field_subscripted_then_array_of_structured() {
    let library = parse_in_method("    count := THIS^.values[2];");
    let assignment = cast!(&first_method_body(&library)[0], StmtKind::Assignment);
    let variable = cast!(&assignment.value.kind, ExprKind::Variable);
    let symbolic = cast!(variable, Variable::Symbolic);
    let array = cast!(symbolic, SymbolicVariableKind::Array);
    let structured = cast!(
        array.subscripted_variable.as_ref(),
        SymbolicVariableKind::Structured
    );
    let self_ref = cast!(structured.record.as_ref(), SymbolicVariableKind::SelfRef);
    assert_eq!(self_ref.kind, SelfRefKind::This);
}

/// `THIS^` with no element chain is a complete variable reference.
#[test]
fn parse_when_bare_this_caret_then_self_ref_alone() {
    let library = parse_in_method("    count := THIS^;");
    let assignment = cast!(&first_method_body(&library)[0], StmtKind::Assignment);
    let variable = cast!(&assignment.value.kind, ExprKind::Variable);
    let symbolic = cast!(variable, Variable::Symbolic);
    let self_ref = cast!(symbolic, SymbolicVariableKind::SelfRef);
    assert_eq!(self_ref.kind, SelfRefKind::This);
}

#[rstest]
#[case::this("    THIS^.Start();", SelfRefKind::This)]
#[case::super_("    SUPER^.Start();", SelfRefKind::Super)]
fn parse_when_self_ref_method_call_then_self_ref_receiver(
    #[case] body: &str,
    #[case] expected: SelfRefKind,
) {
    let library = parse_in_method(body);
    let call = cast!(&first_method_body(&library)[0], StmtKind::MethodCall);
    assert_eq!(call.method, Id::from("Start"));
    let self_ref = cast!(&call.receiver, MethodReceiver::SelfRef);
    assert_eq!(self_ref.kind, expected);
}

#[test]
fn parse_when_self_ref_method_call_has_args_then_params_captured() {
    let library = parse_in_method("    SUPER^.SetSpeed(rNewSpeed := 1.0);");
    let call = cast!(&first_method_body(&library)[0], StmtKind::MethodCall);
    assert_eq!(call.params.len(), 1);
}

/// An ordinary instance receiver still parses -- the receiver rule is
/// additive, not a replacement.
#[test]
fn parse_when_instance_method_call_then_instance_receiver() {
    let library = parse_in_method("    motor.Start();");
    let call = cast!(&first_method_body(&library)[0], StmtKind::MethodCall);
    let instance = cast!(&call.receiver, MethodReceiver::Instance);
    assert_eq!(instance, &Id::from("motor"));
}

/// Whitespace between the keyword and its caret is accepted: nothing in
/// the grammar joins them into a single token. See the plan's
/// "whitespace question" section.
#[rstest]
#[case::no_space("    THIS^.count := 1;")]
#[case::space("    THIS ^.count := 1;")]
#[case::comment("    THIS (* self *) ^.count := 1;")]
fn parse_when_whitespace_before_caret_then_ok(#[case] body: &str) {
    let library = parse_in_method(body);
    let assignment = cast!(&first_method_body(&library)[0], StmtKind::Assignment);
    let symbolic = cast!(&assignment.target, Variable::Symbolic);
    let structured = cast!(symbolic, SymbolicVariableKind::Structured);
    let self_ref = cast!(structured.record.as_ref(), SymbolicVariableKind::SelfRef);
    assert_eq!(self_ref.kind, SelfRefKind::This);
}

/// The caret is mandatory: THIS/SUPER are pointers in the dialects that
/// define them, so the un-dereferenced forms are rejected. A dedicated
/// diagnostic for this shape is tracked in issue #1405.
#[rstest]
#[case::bare_this("    THIS := 1;")]
#[case::this_dot("    THIS.count := 1;")]
#[case::bare_super("    SUPER := 1;")]
#[case::super_dot("    SUPER.count := 1;")]
fn parse_when_self_ref_without_caret_then_err(#[case] body: &str) {
    let source = format!(
        "FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    count : INT;
END_VAR
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
    );
    let result = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance());
    assert!(result.is_err());
}
