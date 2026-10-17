use super::apply;
use crate::test_helpers::{codes, rule_codes, rule_diagnostics};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// `I_Comm` extends `I_Base`, which declares `Reset`. `I_Comm` declares a
/// method `Send` and a read-only property `Ready`.
const INTERFACES: &str = "
INTERFACE I_Base
METHOD Reset
END_METHOD
END_INTERFACE

INTERFACE I_Comm EXTENDS I_Base
METHOD Send : BOOL
VAR_INPUT
    data : INT;
END_VAR
VAR_OUTPUT
    sent : UINT;
END_VAR
END_METHOD
PROPERTY Ready : BOOL
GET END_GET
END_PROPERTY
END_INTERFACE
";

const RESET: &str = "
METHOD Reset
END_METHOD
";

const SEND: &str = "
METHOD Send : BOOL
VAR_INPUT
    data : INT;
END_VAR
VAR_OUTPUT
    sent : UINT;
END_VAR
VAR
    local : DINT;
END_VAR
    Send := TRUE;
END_METHOD
";

const READY: &str = "
PROPERTY Ready : BOOL
GET
    Ready := TRUE;
END_GET
END_PROPERTY
";

fn check(source: &str) -> Vec<String> {
    let program = format!("{INTERFACES}\n{source}");
    rule_codes(apply, &program, &options())
}

fn serial(members: &str) -> String {
    format!("FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm\n{members}\nEND_FUNCTION_BLOCK")
}

fn missing() -> Vec<&'static str> {
    codes(&[Problem::InterfaceMemberMissing])
}

fn mismatch() -> Vec<&'static str> {
    codes(&[Problem::InterfaceMemberMismatch])
}

#[test]
fn apply_when_all_members_provided_then_ok() {
    assert_eq!(codes(&[]), check(&serial(&format!("{RESET}{SEND}{READY}"))));
}

#[test]
fn apply_when_members_inherited_through_extends_then_ok() {
    let source = format!(
        "FUNCTION_BLOCK FB_Base\n{RESET}{SEND}{READY}\nEND_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Serial EXTENDS FB_Base IMPLEMENTS I_Comm
END_FUNCTION_BLOCK"
    );
    assert_eq!(codes(&[]), check(&source));
}

#[test]
fn apply_when_property_has_extra_accessor_then_ok() {
    let ready = "
PROPERTY Ready : BOOL
GET
    Ready := TRUE;
END_GET
SET
END_SET
END_PROPERTY
";
    assert_eq!(codes(&[]), check(&serial(&format!("{RESET}{SEND}{ready}"))));
}

#[rstest]
#[case::method("{RESET}{READY}")]
#[case::method_of_extended_interface("{SEND}{READY}")]
#[case::property("{RESET}{SEND}")]
fn apply_when_member_missing_then_error(#[case] members: &str) {
    let members = members
        .replace("{RESET}", RESET)
        .replace("{SEND}", SEND)
        .replace("{READY}", READY);
    assert_eq!(missing(), check(&serial(&members)));
}

#[test]
fn apply_when_abstract_function_block_misses_member_then_error() {
    let source = format!(
        "FUNCTION_BLOCK ABSTRACT FB_Serial IMPLEMENTS I_Comm\n{RESET}{READY}\nEND_FUNCTION_BLOCK"
    );
    assert_eq!(missing(), check(&source));
}

#[rstest]
#[case::return_type("Send : BOOL", "Send : INT")]
#[case::no_return_type("Send : BOOL", "Send")]
#[case::parameter_name("data : INT;", "value : INT;")]
#[case::parameter_type("data : INT;", "data : DINT;")]
#[case::parameter_kind("VAR_OUTPUT\n    sent", "VAR_IN_OUT\n    sent")]
#[case::extra_parameter("data : INT;", "data : INT;\n    more : INT;")]
fn apply_when_method_signature_differs_then_error(#[case] from: &str, #[case] to: &str) {
    let send = SEND.replacen(from, to, 1);
    assert_eq!(mismatch(), check(&serial(&format!("{RESET}{send}{READY}"))));
}

#[rstest]
#[case::property_type("PROPERTY Ready : BOOL", "PROPERTY Ready : INT")]
#[case::missing_accessor("GET\n    Ready := TRUE;\nEND_GET", "SET\nEND_SET")]
fn apply_when_property_differs_then_error(#[case] from: &str, #[case] to: &str) {
    let ready = READY.replacen(from, to, 1);
    assert_eq!(mismatch(), check(&serial(&format!("{RESET}{SEND}{ready}"))));
}

#[rstest]
#[case::undeclared("I_Unknown")]
#[case::function_block("FB_Plain")]
#[case::structure("S_Point")]
fn apply_when_implements_names_no_interface_then_error(#[case] name: &str) {
    let source = format!(
        "TYPE S_Point : STRUCT x : INT; END_STRUCT; END_TYPE
FUNCTION_BLOCK FB_Plain
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Other IMPLEMENTS {name}
END_FUNCTION_BLOCK"
    );
    assert_eq!(codes(&[Problem::ImplementsNotInterface]), check(&source));
}

#[test]
fn apply_when_implements_interface_and_one_it_extends_then_each_problem_once() {
    let source = format!(
        "FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm, I_Base\n{SEND}{READY}\nEND_FUNCTION_BLOCK"
    );
    assert_eq!(missing(), check(&source));
}

/// `FB_Base` implements `I_Comm` correctly; `FB_Derived` extends it,
/// declaring `{members}`.
fn derived(members: &str) -> String {
    format!(
        "FUNCTION_BLOCK FB_Base IMPLEMENTS I_Comm\n{RESET}{SEND}{READY}\nEND_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base\n{members}\nEND_FUNCTION_BLOCK"
    )
}

#[rstest]
#[case::inherits_members("")]
#[case::overrides_method_alike(SEND)]
#[case::overrides_property_alike(READY)]
fn apply_when_derived_block_inherits_interface_then_ok(#[case] members: &str) {
    assert_eq!(codes(&[]), check(&derived(members)));
}

#[rstest]
#[case::method_return_type(SEND, "Send : BOOL", "Send : INT")]
#[case::method_parameter(SEND, "data : INT;", "data : DINT;")]
#[case::property_type(READY, "Ready : BOOL", "Ready : INT")]
fn apply_when_derived_block_overrides_differently_then_mismatch(
    #[case] member: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let member = member.replacen(from, to, 1);
    assert_eq!(mismatch(), check(&derived(&member)));
}

#[test]
fn apply_when_base_misses_member_then_only_base_reports() {
    let source = format!(
        "FUNCTION_BLOCK FB_Base IMPLEMENTS I_Comm\n{RESET}{READY}\nEND_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK"
    );
    assert_eq!(missing(), check(&source));
}

#[test]
fn apply_when_derived_block_overrides_differently_then_reported_at_member() {
    let send = SEND.replacen("Send : BOOL", "Send : INT", 1);
    let program = format!("{INTERFACES}\n{}", derived(&send));
    let diagnostics = rule_diagnostics(apply, &program, &options());

    assert_eq!(1, diagnostics.len());
    let primary = &diagnostics[0].primary.location;
    assert_eq!("Send", &program[primary.start..primary.end]);
    assert!(primary.start > program.find("FB_Derived EXTENDS").unwrap());
}

/// An interface `I_Shape` with one method whose parameter is `{parameter}`,
/// implemented by a function block whose method declares `{implemented}`.
fn shape_check(parameter: &str, implemented: &str) -> Vec<String> {
    let program = format!(
        "
INTERFACE I_Shape
METHOD M
VAR_INPUT
    {parameter}
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Shape IMPLEMENTS I_Shape
METHOD M
VAR_INPUT
    {implemented}
END_VAR
END_METHOD
END_FUNCTION_BLOCK"
    );
    rule_codes(apply, &program, &options())
}

#[rstest]
#[case::inline_array("x : ARRAY[1..3] OF INT;")]
#[case::sized_string("s : STRING[10];")]
#[case::rising_edge("data : BOOL R_EDGE;")]
fn apply_when_parameter_declared_identically_then_ok(#[case] parameter: &str) {
    assert_eq!(codes(&[]), shape_check(parameter, parameter));
}

#[rstest]
#[case::inline_array_bounds("x : ARRAY[1..3] OF INT;", "x : ARRAY[1..4] OF INT;")]
#[case::inline_array_element("x : ARRAY[1..3] OF INT;", "x : ARRAY[1..3] OF DINT;")]
#[case::string_length("s : STRING[10];", "s : STRING[99];")]
#[case::string_width("s : STRING[10];", "s : WSTRING[10];")]
#[case::edge_input_missing("other : INT;\n    data : BOOL R_EDGE;", "other : INT;")]
#[case::edge_direction("data : BOOL R_EDGE;", "data : BOOL F_EDGE;")]
fn apply_when_parameter_declared_differently_then_mismatch(
    #[case] parameter: &str,
    #[case] implemented: &str,
) {
    assert_eq!(mismatch(), shape_check(parameter, implemented));
}
