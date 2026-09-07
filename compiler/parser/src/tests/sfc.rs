//! Sequential function chart elements (B.1.6). The corpus tests pin the
//! whole `first_steps` chart; these cover the step forms that the corpus
//! fixtures never exercise.

use super::common::*;

/// Regression for <https://github.com/ironplc/ironplc/issues/1659>: the
/// `INITIAL_STEP` rule had its own copy of the step tail that neither
/// consumed the terminating `;` nor allowed whitespace before `END_STEP`,
/// so any association on the initial step was a syntax error.
#[test]
fn parse_when_initial_step_has_action_association_then_builds_step() {
    let library = parse_text(
        "PROGRAM main
VAR x : INT; END_VAR
    INITIAL_STEP Start:
        DoIt(N);
    END_STEP
    TRANSITION FROM Start TO Idle := TRUE; END_TRANSITION
    STEP Idle:
    END_STEP
    ACTION DoIt:
        x := x + 1;
    END_ACTION
END_PROGRAM",
    );

    let expected = Step {
        name: Id::from("Start"),
        action_associations: vec![ActionAssociation::new("DoIt", Some(ActionQualifier::N))],
    };
    assert_eq!(initial_step(&library), &expected);
}

#[test]
fn parse_when_initial_step_has_two_action_associations_then_builds_step() {
    let library = parse_text(
        "PROGRAM main
VAR x : INT; END_VAR
    INITIAL_STEP Start:
        DoIt(N);
        DoIt(P, x);
    END_STEP
    ACTION DoIt:
        x := x + 1;
    END_ACTION
END_PROGRAM",
    );

    let expected = Step {
        name: Id::from("Start"),
        action_associations: vec![
            ActionAssociation::new("DoIt", Some(ActionQualifier::N)),
            ActionAssociation {
                name: Id::from("DoIt"),
                qualifier: Some(ActionQualifier::P),
                indicators: vec![Id::from("x")],
            },
        ],
    };
    assert_eq!(initial_step(&library), &expected);
}

/// The empty `STEP` from the issue's example. `semisep` demanded a `;` even
/// with nothing to terminate, so before the shared rule this was a syntax
/// error too; only the empty `INITIAL_STEP` worked.
#[test]
fn parse_when_step_has_no_action_associations_then_builds_empty_step() {
    let library = parse_text(
        "PROGRAM main
VAR x : INT; END_VAR
    INITIAL_STEP Start:
    END_STEP
    STEP Idle:
    END_STEP
    ACTION DoIt:
        x := x + 1;
    END_ACTION
END_PROGRAM",
    );

    let expected = ElementKind::step(Id::from("Idle"), vec![]);
    assert_eq!(network(&library).elements[0], expected);
}

/// `semisep_or_empty` requires the terminating `;`, on the initial step as on any
/// other, so the spelling without it stays a syntax error.
#[test]
fn parse_when_initial_step_association_missing_semicolon_then_error() {
    let result = parse_program(
        "PROGRAM main
VAR x : INT; END_VAR
    INITIAL_STEP Start:
        DoIt(N)
    END_STEP
    ACTION DoIt:
        x := x + 1;
    END_ACTION
END_PROGRAM",
        &FileId::default(),
        &CompilerOptions::default(),
    );

    assert_eq!(result.unwrap_err().code, "P0002");
}

fn initial_step(library: &Library) -> &Step {
    &network(library).initial_step
}

fn network(library: &Library) -> &Network {
    match &library.elements[0] {
        LibraryElementKind::ProgramDeclaration(program) => match &program.body {
            FunctionBlockBodyKind::Sfc(sfc) => &sfc.networks[0],
            body => panic!("expected an SFC body, found {body:?}"),
        },
        element => panic!("expected a program, found {element:?}"),
    }
}
