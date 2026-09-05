//! Unit tests for `xform_mark_unwritten_constants`: the corner cases of write
//! detection and marking that the requirement conformance tests in
//! `spec_conformance_constant_inference` do not pin.
use super::*;
use crate::test_helpers::{
    declaration_qualifiers, parse_and_resolve_types, parse_and_resolve_types_with_options,
};
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

/// The qualifier of the one declaration named `name`.
fn qualifier(library: &Library, name: &str) -> DeclarationQualifier {
    let mut found = declaration_qualifiers(library, name);
    assert_eq!(1, found.len(), "expected one declaration of {name}");
    found.remove(0)
}

fn oop_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

// -----------------------------------------------------------------
// Initializer kinds
// -----------------------------------------------------------------

#[rstest]
#[case::enumerated_values("x : (Red, Green) := Green;")]
#[case::enumerated_type("x : Color := Green;")]
#[case::array("x : ARRAY[0..1] OF INT := [1, 2];")]
#[case::wstring("x : WSTRING := \"Hi\";")]
fn apply_when_initializer_kind_has_value_then_constant(#[case] decl: &str) {
    let program = format!(
        "
TYPE Color : (Red, Green); END_TYPE
PROGRAM main
VAR
    {decl}
END_VAR
END_PROGRAM"
    );
    let library = parse_and_resolve_types(&program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "x"));
}

#[rstest]
#[case::enumerated_type("x : Color;")]
#[case::array("x : ARRAY[0..1] OF INT;")]
#[case::structure("x : Point := (a := 1);")]
#[case::string("x : STRING;")]
fn apply_when_initializer_kind_lacks_value_then_unchanged(#[case] decl: &str) {
    let program = format!(
        "
TYPE
    Color : (Red, Green);
    Point : STRUCT a : INT; b : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    {decl}
END_VAR
END_PROGRAM"
    );
    let library = parse_and_resolve_types(&program);
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "x"));
}

// -----------------------------------------------------------------
// Call arguments
// -----------------------------------------------------------------

#[test]
fn apply_when_positional_arg_to_fb_in_out_then_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Bump
VAR_INPUT
    step : INT;
END_VAR
VAR_IN_OUT
    total : INT;
END_VAR
    total := total + step;
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Bump;
    delta : INT := 1;
    acc : INT := 0;
END_VAR
    inst(delta, acc);
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "delta"));
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "acc")
    );
}

#[test]
fn apply_when_fb_in_out_declared_on_base_then_derived_call_arg_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Base
VAR_IN_OUT
    total : INT;
END_VAR
    total := total + 1;
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Derived;
    acc : INT := 0;
END_VAR
    inst(total := acc);
END_PROGRAM";
    let (library, _) = parse_and_resolve_types_with_options(program, &oop_options());
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "acc")
    );
}

#[test]
fn apply_when_stdlib_fb_input_arg_then_constant() {
    let program = "
PROGRAM main
VAR
    timer : TON;
    delay : TIME := T#1s;
    start : BOOL := FALSE;
END_VAR
    timer(IN := start, PT := delay);
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "delay"));
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "start"));
}

#[test]
fn apply_when_fb_call_on_undeclared_instance_then_args_unchanged() {
    let program = "
PROGRAM main
VAR
    delay : TIME := T#1s;
END_VAR
    ghost(PT := delay);
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "delay")
    );
}

#[test]
fn apply_when_fb_call_names_unknown_param_then_arg_unchanged() {
    let program = "
PROGRAM main
VAR
    timer : TON;
    delay : TIME := T#1s;
END_VAR
    timer(NOPE := delay);
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "delay")
    );
}

#[test]
fn apply_when_extensible_function_extra_arg_then_constant() {
    let program = "
PROGRAM main
VAR
    a : INT := 1;
    b : INT := 2;
    c : INT := 3;
    sum : INT;
END_VAR
    sum := ADD(a, b, c);
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "c"));
}

#[test]
fn apply_when_method_in_out_arg_then_unchanged_and_input_arg_constant() {
    let program = "
FUNCTION_BLOCK FB_Motor
METHOD Bump
VAR_INPUT
    step : INT;
END_VAR
VAR_IN_OUT
    total : INT;
END_VAR
    total := total + step;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
VAR
    m : FB_Motor;
    delta : INT := 1;
    acc : INT := 0;
END_VAR
    m.Bump(step := delta, total := acc);
END_PROGRAM";
    let (library, _) = parse_and_resolve_types_with_options(program, &oop_options());
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "delta"));
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "acc")
    );
}

#[test]
fn apply_when_undeclared_method_then_args_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Motor
END_FUNCTION_BLOCK
PROGRAM main
VAR
    m : FB_Motor;
    delta : INT := 1;
END_VAR
    m.Nope(delta);
END_PROGRAM";
    let (library, _) = parse_and_resolve_types_with_options(program, &oop_options());
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "delta")
    );
}

// -----------------------------------------------------------------
// Members and instance initializers
// -----------------------------------------------------------------

#[test]
fn apply_when_struct_field_assigned_then_same_named_var_constant() {
    // `p.count` is a structure field, not a function-block member, so the
    // write reaches `p` alone and the variable `count` stays constant.
    let program = "
TYPE Point : STRUCT count : INT; END_STRUCT; END_TYPE
PROGRAM main
VAR
    p : Point;
    count : INT := 5;
END_VAR
    p.count := 1;
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "count"));
}

#[test]
fn apply_when_self_ref_member_assigned_then_member_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT := 0;
END_VAR
METHOD Reset
    THIS^.count := 0;
END_METHOD
END_FUNCTION_BLOCK";
    let (library, _) = parse_and_resolve_types_with_options(program, &oop_options());
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "count")
    );
}

#[test]
fn apply_when_nested_instance_initializer_then_nested_member_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Inner
VAR
    depth : INT := 0;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Outer
VAR
    inner : FB_Inner;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    outer : FB_Outer := (inner := (depth := 3));
END_VAR
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "depth")
    );
}

#[test]
fn apply_when_call_style_initializer_names_member_then_member_unchanged() {
    let program = "
FUNCTION_BLOCK FB_Comm
VAR
    retries : INT := 3;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    comm : FB_Comm(retries := 5);
END_VAR
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "retries")
    );
}

// -----------------------------------------------------------------
// Globals
// -----------------------------------------------------------------

#[test]
fn apply_when_same_named_globals_differ_then_none_marked() {
    let program = "
PROGRAM main
END_PROGRAM
CONFIGURATION config
    VAR_GLOBAL
        limit : INT := 10;
    END_VAR
    RESOURCE res ON PLC
        VAR_GLOBAL
            limit : INT;
        END_VAR
        TASK fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM prog WITH fast : main;
    END_RESOURCE
END_CONFIGURATION";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        vec![
            DeclarationQualifier::Unspecified,
            DeclarationQualifier::Unspecified
        ],
        declaration_qualifiers(&library, "limit")
    );
}

#[test]
fn apply_when_located_var_init_then_variable_unchanged() {
    let program = "
PROGRAM main
VAR
    limit : INT := 10;
END_VAR
END_PROGRAM
CONFIGURATION config
    RESOURCE res ON PLC
        TASK fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM prog WITH fast : main;
    END_RESOURCE
    VAR_CONFIG
        res.prog.limit : INT := 20;
    END_VAR
END_CONFIGURATION";
    let library = parse_and_resolve_types(program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "limit")
    );
}

#[test]
fn apply_when_program_access_read_only_then_constant() {
    let program = "
PROGRAM main
VAR
    limit : INT := 10;
END_VAR
VAR_ACCESS
    LIMIT_VIEW : limit : INT READ_ONLY;
END_VAR
END_PROGRAM";
    let library = parse_and_resolve_types(program);
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "limit"));
}

// -----------------------------------------------------------------
// Callee parameter binding
// -----------------------------------------------------------------

#[test]
fn positional_is_in_out_when_beyond_extensible_params_then_input() {
    let params = CalleeParams {
        params: vec![(Id::from("IN1"), false)],
        extensible: true,
    };
    assert_eq!(Some(false), params.positional_is_in_out(3));
    assert_eq!(Some(false), params.named_is_in_out(&Id::from("IN4")));
}

#[test]
fn positional_is_in_out_when_beyond_fixed_params_then_unknown() {
    let params = CalleeParams {
        params: vec![(Id::from("IN1"), false)],
        extensible: false,
    };
    assert_eq!(None, params.positional_is_in_out(1));
    assert_eq!(None, params.named_is_in_out(&Id::from("IN2")));
}
