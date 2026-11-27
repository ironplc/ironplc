//! Type names written with a library prefix (`BROTLib.E_Blink`). The parser
//! records the prefix; the analyzer checks that it names the library that
//! declares the type.

use crate::options::{CompilerOptions, Dialect};
use crate::parse_program;
use dsl::common::*;
use dsl::core::FileId;
use dsl::textual::{CaseSelectionKind, StmtKind};
use rstest::rstest;

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

fn parse(source: &str) -> Library {
    parse_program(source, &FileId::default(), &twincat())
        .unwrap_or_else(|d| panic!("Parse failed: {d:?}"))
}

/// The prefix `name` was written with, as written.
fn prefix(name: &TypeName) -> Option<String> {
    name.qualifier.as_ref().map(|q| q.original().to_string())
}

fn function_block(lib: &Library) -> &FunctionBlockDeclaration {
    let LibraryElementKind::FunctionBlockDeclaration(fb) = &lib.elements[0] else {
        panic!("expected a function block");
    };
    fb
}

#[test]
fn parse_when_variable_types_have_library_prefix_then_prefix_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Host
VAR
    mode : BROTLib.E_Blink;
    cover : HalfBROT.FB_CoverControl(comm := fbComm);
    fbComm : FB_Comm;
END_VAR
END_FUNCTION_BLOCK",
    );

    let prefixes: Vec<(Option<String>, String)> = function_block(&lib)
        .variables
        .iter()
        .map(|var| match var.initializer.type_reference() {
            TypeReference::Named(name) => (prefix(&name), name.id.original().to_string()),
            other => panic!("expected a named type, got {other:?}"),
        })
        .collect();
    assert_eq!(
        prefixes,
        [
            (Some("BROTLib".into()), "E_Blink".into()),
            (Some("HalfBROT".into()), "FB_CoverControl".into()),
            (None, "FB_Comm".into()),
        ]
    );
}

#[test]
fn parse_when_extends_and_implements_have_library_prefix_then_prefix_recorded() {
    let lib = parse(
        "FUNCTION_BLOCK FB_Suite EXTENDS TcUnit.FB_TestSuite IMPLEMENTS BROTLib.I_Comm, I_Other
END_FUNCTION_BLOCK",
    );

    let oop = function_block(&lib).oop.as_ref().unwrap();
    assert_eq!(prefix(oop.base.as_ref().unwrap()), Some("TcUnit".into()));
    let implements: Vec<Option<String>> = oop.implements.iter().map(prefix).collect();
    assert_eq!(implements, [Some("BROTLib".into()), None]);
}

#[test]
fn parse_when_prefixed_name_then_equal_to_unprefixed_name() {
    let lib = parse("FUNCTION_BLOCK FB_A EXTENDS Tc2_Standard.TON END_FUNCTION_BLOCK");

    let base = function_block(&lib)
        .oop
        .as_ref()
        .unwrap()
        .base
        .as_ref()
        .unwrap();
    assert_eq!(base, &TypeName::from("TON"));
}

#[test]
fn parse_when_dotted_initial_value_has_library_prefix_then_prefix_on_value_type() {
    let lib = parse(
        "PROGRAM main
VAR
    mode : BROTLib.E_Blink := BROTLib.E_Blink.slow;
END_VAR
END_PROGRAM",
    );

    let LibraryElementKind::ProgramDeclaration(p) = &lib.elements[0] else {
        panic!("expected a program");
    };
    let InitialValueAssignmentKind::LateResolvedType(late) = &p.variables[0].initializer else {
        panic!("expected a late-resolved initializer");
    };
    let Some(LateResolvedInitialValue::Dotted(value)) = &late.initial_value else {
        panic!("expected a dotted initial value");
    };
    let written = value.type_name.as_ref().unwrap();
    assert_eq!(prefix(&late.type_name), Some("BROTLib".into()));
    assert_eq!(prefix(written), Some("BROTLib".into()));
    assert_eq!(written.id.original().as_str(), "E_Blink");
    assert_eq!(value.value.original().as_str(), "slow");
}

#[test]
fn parse_when_case_label_has_library_prefix_then_dotted_value_with_prefixed_type() {
    let lib = parse(
        "PROGRAM main
VAR
    mode : E_Blink;
END_VAR
CASE mode OF
    BROTLib.E_Blink.slow, E_Blink.off: ;
END_CASE;
END_PROGRAM",
    );

    let LibraryElementKind::ProgramDeclaration(p) = &lib.elements[0] else {
        panic!("expected a program");
    };
    let FunctionBlockBodyKind::Statements(body) = &p.body else {
        panic!("expected a statement body");
    };
    let StmtKind::Case(case) = &body.body[0] else {
        panic!("expected a CASE statement");
    };
    let labels: Vec<(Option<String>, String, String)> = case.statement_groups[0]
        .selectors
        .iter()
        .map(|selector| {
            let CaseSelectionKind::EnumeratedValue(value) = selector else {
                panic!("expected an enumerated value label, got {selector:?}");
            };
            let written = value.type_name.as_ref().unwrap();
            (
                prefix(written),
                written.id.original().to_string(),
                value.value.original().to_string(),
            )
        })
        .collect();
    assert_eq!(
        labels,
        [
            (Some("BROTLib".into()), "E_Blink".into(), "slow".into()),
            (None, "E_Blink".into(), "off".into()),
        ]
    );
}

// A declaration names the type it declares, so the name takes no prefix.
// A prefix of more than one name (`A.B.T`) is not a library prefix.
#[rstest]
#[case::enumeration("TYPE Lib.E : (a, b); END_TYPE")]
#[case::structure("TYPE Lib.S : STRUCT x : INT; END_STRUCT; END_TYPE")]
#[case::alias("TYPE Lib.T : INT; END_TYPE")]
#[case::array("TYPE Lib.A : ARRAY[1..2] OF INT; END_TYPE")]
#[case::string("TYPE Lib.Str : STRING[10]; END_TYPE")]
#[case::function_block("FUNCTION_BLOCK Lib.FB_A END_FUNCTION_BLOCK")]
#[case::two_name_prefix("PROGRAM main VAR x : Outer.Inner.FB_A; END_VAR END_PROGRAM")]
fn parse_when_name_takes_no_library_prefix_then_syntax_error(#[case] source: &str) {
    let unprefixed = source.replace("Lib.", "").replace("Outer.Inner.", "Outer.");
    parse(&unprefixed);

    let result = parse_program(source, &FileId::default(), &twincat());
    assert_eq!(result.expect_err("must be rejected").code, "P0002");
}
