//! A P9999 raised by codegen names the construct the compiler cannot handle
//! yet, so the CLI can show the offending line rather than `┌─ :1:1` (issue
//! #1734). Each case is a program that `check` accepts and `compile` refuses;
//! the assertion is on where the primary label lands.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

#[rstest]
#[case::direct_address_write(
    "
PROGRAM main
    %QX0.0 := TRUE;
END_PROGRAM
",
    "%QX0.0",
    "%QX0.0"
)]
#[case::string_return_type_of_method(
    "
FUNCTION_BLOCK FB_Motor
    METHOD Name : STRING
        Name := 'motor';
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
    m();
END_PROGRAM
",
    "STRING\n",
    "STRING"
)]
#[case::property_read(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    x : REAL;
END_VAR
    x := m.Speed;
END_PROGRAM
",
    "Speed;\nEND_PROGRAM",
    "Speed"
)]
#[case::property_write(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
SET
    _speed := Speed;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
    m.Speed := 2.0;
END_PROGRAM
",
    "Speed := 2.0",
    "Speed"
)]
fn compile_when_not_implemented_then_primary_label_names_the_construct(
    #[case] source: &str,
    #[case] anchor: &str,
    #[case] labelled: &str,
) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(source, &options);

    assert!(result.is_err());
    let diagnostic = result.unwrap_err();
    assert_eq!(diagnostic.code, "P9999");
    let start = source.find(anchor).unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + labelled.len());
    assert!(
        diagnostic.source_file.is_some(),
        "the compiler location is still recorded for the telemetry dashboards"
    );
}
