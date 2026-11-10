//! Initial values of `VAR_GLOBAL` variables that are strings or structures.
//!
//! A global goes through `located_var_spec_init`, which must give a string or
//! structure the same initializer kind as the same declaration in a `VAR`
//! block, so later stages treat both alike.

use super::common::*;
use ironplc_dsl::common::StringInitializer;

fn globals_of(globals: &str) -> Vec<VarDecl> {
    let source = format!(
        "TYPE POINT : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
CONFIGURATION config
  VAR_GLOBAL
{globals}
  END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let config = library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::ConfigurationDeclaration(c) => Some(c.clone()),
            _ => None,
        })
        .unwrap();
    config.global_var
}

fn string_initializer(decl: &VarDecl) -> &StringInitializer {
    match &decl.initializer {
        InitialValueAssignmentKind::String(s) => s,
        other => panic!("expected a string initializer, found {other:?}"),
    }
}

#[test]
fn parse_when_global_string_with_length_and_value_then_string_initializer() {
    let globals = globals_of("    s : STRING[10] := 'abc';");
    let init = string_initializer(&globals[0]);
    assert_eq!(init.width, StringType::String);
    assert!(init.length.is_some());
    assert!(init.initial_value.is_some());
}

#[test]
fn parse_when_global_string_with_value_only_then_string_initializer() {
    let globals = globals_of("    s : STRING := 'abc';");
    let init = string_initializer(&globals[0]);
    assert!(init.length.is_none());
    assert!(init.initial_value.is_some());
}

#[test]
fn parse_when_global_string_without_value_then_string_initializer() {
    let globals = globals_of("    s : STRING[10];");
    assert!(string_initializer(&globals[0]).initial_value.is_none());
}

#[test]
fn parse_when_global_wstring_with_length_and_value_then_wide_string_initializer() {
    let globals = globals_of("    w : WSTRING[10] := \"abc\";");
    assert_eq!(string_initializer(&globals[0]).width, StringType::WString);
}

#[test]
fn parse_when_global_structure_with_initial_value_then_members_recorded() {
    let globals = globals_of("    origin : POINT := (x := 3, y := 4);");
    match &globals[0].initializer {
        InitialValueAssignmentKind::LateResolvedType(init) => match &init.initial_value {
            Some(LateResolvedInitialValue::Members(members)) => assert_eq!(members.len(), 2),
            other => panic!("expected member initializers, found {other:?}"),
        },
        other => panic!("expected a late resolved initializer, found {other:?}"),
    }
}

#[test]
fn parse_when_global_structure_without_value_then_no_members() {
    let globals = globals_of("    origin : POINT;");
    assert!(!matches!(
        globals[0].initializer,
        InitialValueAssignmentKind::String(_)
    ));
}

fn external_initializer(source_decl: &str) -> InitialValueAssignmentKind {
    let source = format!(
        "PROGRAM main
  VAR_EXTERNAL
    {source_decl}
  END_VAR
END_PROGRAM"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    program.variables[0].initializer.clone()
}

#[test]
fn parse_when_external_string_with_length_then_string_initializer() {
    match external_initializer("s : STRING[10];") {
        InitialValueAssignmentKind::String(init) => {
            assert_eq!(init.width, StringType::String);
            assert!(init.length.is_some());
            assert!(init.initial_value.is_none());
        }
        other => panic!("expected a string initializer, found {other:?}"),
    }
}

#[test]
fn parse_when_external_wstring_with_length_then_wide_string_initializer() {
    match external_initializer("w : WSTRING[10];") {
        InitialValueAssignmentKind::String(init) => assert_eq!(init.width, StringType::WString),
        other => panic!("expected a string initializer, found {other:?}"),
    }
}

#[test]
fn parse_when_external_string_without_length_then_string_initializer() {
    match external_initializer("s : STRING;") {
        InitialValueAssignmentKind::String(init) => assert!(init.length.is_none()),
        other => panic!("expected a string initializer, found {other:?}"),
    }
}
