//! Enumerated values written `Type.value`.

use crate::options::{CompilerOptions, Dialect};
use crate::parse_program;
use dsl::common::*;
use dsl::core::FileId;
use dsl::diagnostic::Diagnostic;
use dsl::textual::{CaseSelectionKind, StmtKind};

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

fn parse(source: &str) -> Library {
    parse_program(source, &FileId::default(), &twincat())
        .unwrap_or_else(|d| panic!("Parse failed: {d:?}"))
}

/// The diagnostic a parse of `source` fails with.
fn parse_err(source: &str) -> Diagnostic {
    parse_program(source, &FileId::default(), &twincat()).expect_err("the source must be rejected")
}

// A member declaration names a value, so it has no type to write before it.
#[test]
fn parse_when_enumeration_member_is_dotted_then_syntax_error() {
    assert_eq!(
        parse_err("TYPE E_Blink : (E_Blink.off, slow); END_TYPE").code,
        "P0002"
    );
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
    e_blink.fast, E_Blink#off: out := 2;
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
    let labels: Vec<(String, String, bool)> = case.statement_groups[0]
        .selectors
        .iter()
        .map(|selector| {
            let CaseSelectionKind::EnumeratedValue(value) = selector else {
                panic!("expected an enumerated value label, got {selector:?}");
            };
            let written = value.type_name.as_ref().unwrap();
            (
                written.id.original().to_string(),
                value.value.original().to_string(),
                value.dotted,
            )
        })
        .collect();
    assert_eq!(
        labels,
        [
            ("e_blink".into(), "fast".into(), true),
            ("E_Blink".into(), "off".into(), false),
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
fbOn(blink := E_Blink.on);
mode := E_Blink.on;
END_FUNCTION_BLOCK",
    );
}
