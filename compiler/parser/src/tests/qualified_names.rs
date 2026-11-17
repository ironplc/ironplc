//! Names written with a library or namespace prefix, and enumerated values
//! written `Type.value`.

use crate::options::{CompilerOptions, Dialect};
use crate::parse_program;
use dsl::common::*;
use dsl::core::FileId;
use dsl::textual::{CaseSelectionKind, StmtKind};

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

fn parse(source: &str) -> Library {
    parse_program(source, &FileId::default(), &twincat())
        .unwrap_or_else(|d| panic!("Parse failed: {d:?}"))
}

fn qualifier_text(name: &TypeName) -> Vec<String> {
    name.qualifier
        .iter()
        .map(|q| q.original().to_string())
        .collect()
}

#[test]
fn parse_when_extends_has_library_prefix_then_qualifier_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Suite EXTENDS TcUnit.FB_TestSuite
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    let base = fb.oop.as_ref().and_then(|o| o.base.as_ref()).unwrap();
    assert_eq!(base.id.original().as_str(), "FB_TestSuite");
    assert_eq!(qualifier_text(base), ["TcUnit"]);
}

#[test]
fn parse_when_implements_has_library_prefix_then_qualifier_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Host IMPLEMENTS BROTLib.I_Comm, I_Other
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    let interfaces = &fb.oop.as_ref().unwrap().implements;
    assert_eq!(qualifier_text(&interfaces[0]), ["BROTLib"]);
    assert!(interfaces[1].qualifier.is_empty());
}

#[test]
fn parse_when_name_has_two_prefix_segments_then_both_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Suite EXTENDS Outer.Inner.FB_Base
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    let base = fb.oop.as_ref().and_then(|o| o.base.as_ref()).unwrap();
    assert_eq!(qualifier_text(base), ["Outer", "Inner"]);
}

#[test]
fn parse_when_qualified_name_differs_only_by_prefix_then_equal_to_flat_name() {
    let lib = parse(
        "FUNCTION_BLOCK FB_A EXTENDS Lib.FB_Base
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    let base = fb.oop.as_ref().and_then(|o| o.base.as_ref()).unwrap();
    assert_eq!(base, &TypeName::from("FB_Base"));
}

#[test]
fn parse_when_variable_type_has_library_prefix_then_qualifier_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Host
VAR
    eMode : BROTLib.E_Blink;
    cover : HalfBROT.FB_CoverControl(comm := fbComm);
    fbComm : FB_Comm;
END_VAR
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    let prefixes: Vec<Vec<String>> = fb
        .variables
        .iter()
        .map(|var| match var.initializer.type_reference() {
            TypeReference::Named(name) => qualifier_text(&name),
            other => panic!("expected a named type, got {other:?}"),
        })
        .collect();
    // The call-style initializer keeps the prefix like a plain type does.
    assert_eq!(
        prefixes,
        [vec!["BROTLib"], vec!["HalfBROT"], Vec::<&str>::new()]
    );
}

#[test]
fn parse_when_prefix_has_no_final_name_then_err() {
    let result = parse_program(
        "FUNCTION_BLOCK FB_Suite EXTENDS TcUnit.
END_FUNCTION_BLOCK",
        &FileId::default(),
        &twincat(),
    );

    assert!(result.is_err());
}

#[test]
fn parse_when_initial_value_is_dotted_enumerated_value_with_library_prefix_then_prefix_on_value_type(
) {
    let lib = parse(
        "TYPE E_RoofState : (unknown, open, closed); END_TYPE

FUNCTION_BLOCK FB_Roof
VAR
    other : Lib.E_RoofState := Lib.E_RoofState.closed;
END_VAR
END_FUNCTION_BLOCK",
    );

    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[1] else {
        panic!("expected a function block");
    };
    let InitialValueAssignmentKind::LateResolvedType(late) = &fb.variables[0].initializer else {
        panic!("expected a late-resolved initializer");
    };
    let Some(LateResolvedInitialValue::Dotted(value)) = &late.initial_value else {
        panic!("expected a dotted initial value");
    };
    let written = value.type_name.as_ref().unwrap();
    assert_eq!(qualifier_text(&late.type_name), ["Lib"]);
    assert_eq!(qualifier_text(written), ["Lib"]);
    assert_eq!(written.id.original().as_str(), "E_RoofState");
    assert_eq!(value.value.original().as_str(), "closed");
}

#[test]
fn parse_when_case_label_is_dotted_enumerated_value_then_dotted_value_with_type() {
    let lib = parse(
        "TYPE E_Blink : (off, slow, fast); END_TYPE

PROGRAM main
VAR
    mode : E_Blink;
    out : INT;
END_VAR
CASE mode OF
    Lib.e_blink.fast, E_Blink#off: out := 2;
END_CASE;
END_PROGRAM",
    );

    let LibraryElementKind::ProgramDeclaration(p) = &lib.elements[1] else {
        panic!("expected a program");
    };
    let FunctionBlockBodyKind::Statements(body) = &p.body else {
        panic!("expected a statement body");
    };
    let StmtKind::Case(case) = &body.body[0] else {
        panic!("expected a CASE statement");
    };
    let labels: Vec<(Vec<String>, String, String, bool)> = case.statement_groups[0]
        .selectors
        .iter()
        .map(|selector| {
            let CaseSelectionKind::EnumeratedValue(value) = selector else {
                panic!("expected an enumerated value label, got {selector:?}");
            };
            let written = value.type_name.as_ref().unwrap();
            (
                qualifier_text(written),
                written.id.original().to_string(),
                value.value.original().to_string(),
                value.dotted,
            )
        })
        .collect();
    assert_eq!(
        labels,
        [
            (
                vec!["Lib".to_string()],
                "e_blink".into(),
                "fast".into(),
                true
            ),
            (vec![], "E_Blink".into(), "off".into(), false),
        ]
    );
}

#[test]
fn parse_when_enumerated_value_uses_dot_then_left_for_resolver_with_dotted_flag() {
    let lib = parse(
        "TYPE E_Blink : (off, slow); END_TYPE

PROGRAM main
VAR
    a : E_Blink := E_Blink.slow;
    b : E_Blink := E_Blink#slow;
END_VAR
END_PROGRAM",
    );

    let LibraryElementKind::ProgramDeclaration(p) = &lib.elements[1] else {
        panic!("expected a program");
    };
    // `Type.value` may be a constant reached through a name, so the resolver
    // decides; `Type#value` can only be an enumerated value.
    let InitialValueAssignmentKind::LateResolvedType(late) = &p.variables[0].initializer else {
        panic!("expected a late-resolved initializer");
    };
    let Some(LateResolvedInitialValue::Dotted(dotted)) = &late.initial_value else {
        panic!("expected a dotted initial value");
    };
    let InitialValueAssignmentKind::EnumeratedType(hashed) = &p.variables[1].initializer else {
        panic!("expected an enumerated initializer");
    };
    let hashed = hashed.initial_value.clone().unwrap();
    assert!(dotted.dotted);
    assert!(!hashed.dotted);
    assert_eq!(dotted, &hashed);
}

#[test]
fn parse_when_keyword_is_field_name_after_dot_then_ok() {
    parse(
        "FUNCTION_BLOCK FB_Host
VAR
    fbOn : FB_Blinker;
    mode : E_Blink;
END_VAR
fbOn(blink := Lib.E_Blink.on);
mode := E_Blink.on;
END_FUNCTION_BLOCK",
    );
}
