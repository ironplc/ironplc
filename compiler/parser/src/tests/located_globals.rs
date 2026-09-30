//! Located variables (`name AT %QX0.0 : BOOL`) in `VAR_GLOBAL` blocks.
//!
//! IEC 61131-3 Ed. 2, B.1.4.3 allows a global declaration to be located:
//! `global_var_spec ::= global_var_list | [global_var_name] location`. These
//! tests pin the shape the parser gives it: the same
//! [`VariableIdentifier::Direct`] a located `VAR` declaration gets, declared
//! as [`VariableType::Global`].

use super::common::*;
use dsl::common::{LocationPrefix, SizePrefix};

/// Wraps `globals` in the `VAR_GLOBAL` block of a configuration.
fn in_configuration(globals: &str) -> String {
    format!(
        "CONFIGURATION config
  VAR_GLOBAL
{globals}
  END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION"
    )
}

fn configuration_globals(source: &str) -> Vec<VarDecl> {
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let config = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    config.global_var.clone()
}

#[test]
fn parse_when_configuration_global_located_then_keeps_name_and_address() {
    let globals = configuration_globals(&in_configuration("    lamp AT %QX0.0 : BOOL;"));

    assert_eq!(globals.len(), 1);
    assert_eq!(globals[0].var_type, VariableType::Global);
    let direct = cast!(&globals[0].identifier, VariableIdentifier::Direct);
    assert_eq!(direct.name, Some(Id::from("lamp")));
    assert_eq!(direct.address_assignment.location, LocationPrefix::Q);
    assert_eq!(direct.address_assignment.size, SizePrefix::X);
    assert_eq!(direct.address_assignment.address, vec![0, 0]);
}

#[test]
fn parse_when_configuration_global_located_with_initial_value_then_keeps_initializer() {
    let globals = configuration_globals(&in_configuration("    level AT %IW2 : INT := 7;"));

    let init = cast!(&globals[0].initializer, InitialValueAssignmentKind::Simple);
    assert_eq!(init.type_name, TypeName::from("INT"));
    assert!(init.initial_value.is_some());
}

#[test]
fn parse_when_configuration_global_located_without_name_then_direct_without_name() {
    let globals = configuration_globals(&in_configuration("    AT %MW4 : INT;"));

    let direct = cast!(&globals[0].identifier, VariableIdentifier::Direct);
    assert_eq!(direct.name, None);
    assert_eq!(direct.address_assignment.location, LocationPrefix::M);
}

#[test]
fn parse_when_configuration_globals_mix_located_and_symbolic_then_keeps_order() {
    let globals = configuration_globals(&in_configuration(
        "    a, b : INT;
    lamp AT %QX0.1 : BOOL;
    c : BOOL;",
    ));

    let names: Vec<String> = globals
        .iter()
        .map(|decl| decl.identifier.symbolic_id().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["a", "b", "lamp", "c"]);
    assert!(matches!(
        globals[2].identifier,
        VariableIdentifier::Direct(_)
    ));
    assert!(globals
        .iter()
        .all(|decl| decl.var_type == VariableType::Global));
}

#[test]
fn parse_when_configuration_global_block_retain_located_then_qualifier_applied() {
    let source = "CONFIGURATION config
  VAR_GLOBAL RETAIN
    lamp AT %QX0.0 : BOOL;
  END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION";
    let globals = configuration_globals(source);

    assert_eq!(globals[0].qualifier, DeclarationQualifier::Retain);
}

#[test]
fn parse_when_resource_global_located_then_keeps_name_and_address() {
    let source = "CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count AT %MW4 : INT;
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let config = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let globals = &config.resource_decl[0].global_vars;

    assert_eq!(globals[0].var_type, VariableType::Global);
    let direct = cast!(&globals[0].identifier, VariableIdentifier::Direct);
    assert_eq!(direct.name, Some(Id::from("count")));
    assert_eq!(direct.address_assignment.location, LocationPrefix::M);
    assert_eq!(direct.address_assignment.size, SizePrefix::W);
}
