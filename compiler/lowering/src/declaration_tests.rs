//! Tests of what lowering keeps of each declaration: the section of a
//! variable, and the direct address and retention of a variable or field
//! (REQ-LOW-lowering-006, REQ-LOW-lowering-023, REQ-LOW-lowering-024).

use ironplc_ir::lowered::{
    AddressArea, AddressSize, DirectAddress, Field, Retention, Section, TypeKind,
};

use crate::test_support::{lower, Outcome};

/// The section of each variable lowering allocated, by name.
fn sections(outcome: &Outcome) -> Vec<(String, Section)> {
    outcome
        .variables
        .iter()
        .map(|variable| (variable.name().to_string(), variable.section()))
        .collect()
}

/// The field `name` of the instance the body's instance parameter `THIS`
/// refers to.
fn instance_field<'o>(outcome: &'o Outcome, name: &str) -> &'o Field {
    let this = outcome
        .variables
        .iter()
        .find(|variable| variable.name().to_string() == "THIS")
        .expect("the body has an instance parameter");
    let (instance, _) = outcome.types.reference(this.ty()).unwrap();
    outcome
        .types
        .fields(instance)
        .unwrap()
        .iter()
        .find(|field| field.name.to_string() == name)
        .expect("the instance has the field")
}

/// REQ-LOW-lowering-006: each variable of a function carries the section
/// that declares it, its result an output.
#[test]
fn lower_statement_when_function_variables_then_each_has_its_section() {
    let outcome = lower(
        "FUNCTION F : DINT
         VAR_INPUT a : DINT; END_VAR
         VAR_IN_OUT b : DINT; END_VAR
         VAR_OUTPUT o : DINT; END_VAR
         VAR c : DINT; END_VAR
         VAR_TEMP t : DINT; END_VAR
            c := a + b;
            t := c;
            o := t;
            F := o;
         END_FUNCTION",
        "F",
    );

    let sections = sections(&outcome);
    for (name, section) in [
        ("a", Section::Input),
        ("b", Section::InOut),
        ("c", Section::Local),
        ("t", Section::Temporary),
        ("o", Section::Output),
        ("F", Section::Output),
    ] {
        assert!(
            sections.contains(&(name.to_string(), section)),
            "{name} is not a {section:?}: {sections:?}"
        );
    }
}

/// REQ-LOW-lowering-006, REQ-LOW-lowering-024: a body's instance parameter is
/// an `Instance`, and a global reached through a `VAR_EXTERNAL` is a `Global`
/// declared as the global is, `RETAIN` included.
#[test]
fn lower_statement_when_program_reaches_global_then_instance_and_global_sections() {
    let outcome = lower(
        "CONFIGURATION config
           VAR_GLOBAL RETAIN g : DINT; END_VAR
           RESOURCE res ON PLC
             TASK t (INTERVAL := T#10ms, PRIORITY := 1);
             PROGRAM inst WITH t : main;
           END_RESOURCE
         END_CONFIGURATION
         PROGRAM main
         VAR_EXTERNAL g : DINT; END_VAR
         VAR x : DINT; END_VAR
            x := g;
         END_PROGRAM",
        "main",
    );

    assert_eq!(
        sections(&outcome),
        [
            ("THIS".to_string(), Section::Instance),
            ("g".to_string(), Section::Global)
        ]
    );
    let global = &outcome.variables[1];
    assert_eq!(global.retention(), Retention::Retain);
    assert_eq!(global.location(), None);
}

/// REQ-LOW-lowering-023, REQ-LOW-lowering-024: a program's located or
/// retained variable is a field of its instance that carries its address or
/// retention.
#[test]
fn lower_statement_when_program_variable_located_or_retained_then_field_keeps_it() {
    let outcome = lower(
        "PROGRAM main
         VAR x AT %IX0.1 : BOOL; END_VAR
         VAR RETAIN r : DINT; END_VAR
         VAR plain : DINT; END_VAR
            r := r + 1;
         END_PROGRAM",
        "main",
    );

    let located = instance_field(&outcome, "x");
    assert_eq!(
        located.location,
        Some(DirectAddress {
            area: AddressArea::Input,
            size: AddressSize::Bit,
            path: vec![0, 1],
        })
    );
    assert_eq!(located.retention, Retention::None);
    assert_eq!(instance_field(&outcome, "r").retention, Retention::Retain);
    assert_eq!(instance_field(&outcome, "plain").location, None);
    assert_eq!(instance_field(&outcome, "plain").retention, Retention::None);
}

/// REQ-LOW-lowering-024: a function block's retained variable is a field of
/// its type that carries its retention.
#[test]
fn lower_statement_when_block_variable_retained_then_block_field_keeps_it() {
    let outcome = lower(
        "FUNCTION_BLOCK Counter
         VAR RETAIN n : DINT; END_VAR
         VAR_INPUT step : DINT; END_VAR
            n := n + step;
         END_FUNCTION_BLOCK",
        "Counter",
    );

    assert_eq!(instance_field(&outcome, "n").retention, Retention::Retain);
    assert_eq!(instance_field(&outcome, "step").retention, Retention::None);
    let this = &outcome.variables[0];
    let (block, _) = outcome.types.reference(this.ty()).unwrap();
    assert!(matches!(
        outcome.types.kind(block),
        Some(TypeKind::FunctionBlock { .. })
    ));
}
