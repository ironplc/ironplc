use super::apply;
use crate::test_helpers::parse_and_resolve_types_with_options;
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

fn check(source: &str) -> Result<(), Vec<String>> {
    let program = format!("{INTERFACES}\n{source}");
    let (library, context) = parse_and_resolve_types_with_options(&program, &options());
    apply(&library, &context, &options())
        .map_err(|errors| errors.iter().map(|e| e.code.clone()).collect())
}

fn serial(members: &str) -> String {
    format!("FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm\n{members}\nEND_FUNCTION_BLOCK")
}

fn missing() -> Result<(), Vec<String>> {
    Err(vec![Problem::InterfaceMemberMissing.code().to_string()])
}

fn mismatch() -> Result<(), Vec<String>> {
    Err(vec![Problem::InterfaceMemberMismatch.code().to_string()])
}

#[test]
fn apply_when_all_members_provided_then_ok() {
    assert_eq!(Ok(()), check(&serial(&format!("{RESET}{SEND}{READY}"))));
}

#[test]
fn apply_when_members_inherited_through_extends_then_ok() {
    let source = format!(
        "FUNCTION_BLOCK FB_Base\n{RESET}{SEND}{READY}\nEND_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Serial EXTENDS FB_Base IMPLEMENTS I_Comm
END_FUNCTION_BLOCK"
    );
    assert_eq!(Ok(()), check(&source));
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
    assert_eq!(Ok(()), check(&serial(&format!("{RESET}{SEND}{ready}"))));
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

#[test]
fn apply_when_interface_not_declared_then_ok() {
    assert_eq!(
        Ok(()),
        check("FUNCTION_BLOCK FB_Other IMPLEMENTS I_Unknown\nEND_FUNCTION_BLOCK")
    );
}
