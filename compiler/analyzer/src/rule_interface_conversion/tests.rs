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

/// Declarations shared by every case: `I_Comm` extends `I_Base`,
/// `FB_Serial` implements `I_Comm`, `FB_Usb` extends `FB_Serial`, and
/// `FB_Other` implements nothing. `FB_Device` takes an `I_Comm` input and
/// has a method that does too.
const DECLARATIONS: &str = "
INTERFACE I_Base
END_INTERFACE

INTERFACE I_Comm EXTENDS I_Base
END_INTERFACE

FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Usb EXTENDS FB_Serial
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Other
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Device
VAR_INPUT
    comm : I_Comm;
END_VAR
METHOD Attach
VAR_INPUT
    link : I_Comm;
END_VAR
END_METHOD
END_FUNCTION_BLOCK
";

fn check(body: &str) -> Result<(), Vec<String>> {
    let program = format!(
        "{DECLARATIONS}
PROGRAM main
VAR
    comm : I_Comm;
    base : I_Base;
    serial : FB_Serial;
    usb : FB_Usb;
    other : FB_Other;
    device : FB_Device;
    count : INT;
END_VAR
{body}
END_PROGRAM"
    );
    let (library, context) = parse_and_resolve_types_with_options(&program, &options());
    apply(&library, &context, &options())
        .map_err(|errors| errors.iter().map(|e| e.code.clone()).collect())
}

#[rstest]
#[case::implementing_instance("comm := serial;")]
#[case::instance_of_derived_block("comm := usb;")]
#[case::extended_interface_from_instance("base := serial;")]
#[case::extended_interface_from_interface("base := comm;")]
#[case::same_interface("comm := comm;")]
#[case::named_argument("device(comm := serial);")]
#[case::method_argument("device.Attach(serial);")]
#[case::zero("comm := 0;")]
#[case::zero_argument("device(comm := 0);")]
fn apply_when_value_converts_then_ok(#[case] body: &str) {
    assert_eq!(Ok(()), check(body));
}

#[rstest]
#[case::instance_not_implementing("comm := other;")]
#[case::base_interface_to_derived("comm := base;")]
#[case::elementary_value("comm := count;")]
#[case::nonzero_literal("comm := 1;")]
#[case::named_argument("device(comm := other);")]
#[case::method_argument("device.Attach(other);")]
fn apply_when_value_does_not_convert_then_error(#[case] body: &str) {
    assert_eq!(
        Err(vec![Problem::InterfaceNotImplemented.code().to_string()]),
        check(body)
    );
}

#[test]
fn apply_when_assignment_to_function_block_instance_then_not_checked_here() {
    assert_eq!(Ok(()), check("serial := serial;"));
}

#[rstest]
#[case::method_local_shadows_interface_variable(
    "
METHOD Count
VAR
    comm : INT;
END_VAR
    comm := 5;
END_METHOD"
)]
#[case::method_parameter_shadows_interface_variable(
    "
METHOD Count
VAR_INPUT
    comm : INT;
END_VAR
    comm := 5;
END_METHOD"
)]
fn apply_when_method_declares_same_name_with_other_type_then_ok(#[case] method: &str) {
    let program = format!(
        "{DECLARATIONS}
FUNCTION_BLOCK FB_Holder
VAR
    comm : I_Comm;
END_VAR
{method}
END_FUNCTION_BLOCK"
    );
    let (library, context) = parse_and_resolve_types_with_options(&program, &options());
    let result = apply(&library, &context, &options());
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn apply_when_method_returns_then_block_interface_variable_is_checked_again() {
    let program = format!(
        "{DECLARATIONS}
FUNCTION_BLOCK FB_Holder
VAR
    comm : I_Comm;
    other : FB_Other;
END_VAR
METHOD Count
VAR
    comm : INT;
END_VAR
    comm := 5;
END_METHOD
METHOD Attach
    comm := other;
END_METHOD
END_FUNCTION_BLOCK"
    );
    let (library, context) = parse_and_resolve_types_with_options(&program, &options());
    let errors = apply(&library, &context, &options()).unwrap_err();
    assert_eq!(1, errors.len(), "{errors:?}");
    assert_eq!(Problem::InterfaceNotImplemented.code(), errors[0].code);
}
