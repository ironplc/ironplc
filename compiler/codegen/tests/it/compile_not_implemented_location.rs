//! A P9999 raised by codegen points at the construct, or the statement, the
//! compiler cannot handle yet, so the CLI can show the offending line rather
//! than `┌─ :1:1` (issue #1734). A statement lowering does not support yet is
//! labelled where its span is: an assignment's is its `:=`. Each case is a
//! program that `check` accepts and `compile` refuses; the assertion is on
//! where the primary label lands.

use crate::common::try_parse_and_compile;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

#[rstest]
#[case::direct_address_write(
    "
PROGRAM main
    %QX0.0 := TRUE;
END_PROGRAM
",
    ":= TRUE",
    ":="
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
    "Name : STRING",
    "Name"
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
    ":= m.Speed",
    ":="
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
    ":= 2.0",
    ":="
)]
fn compile_when_not_implemented_then_primary_label_on_its_line(
    #[case] source: &str,
    #[case] anchor: &str,
    #[case] labelled: &str,
) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(source, &options);

    assert_labels(source, result, anchor, labelled);
}

/// Fails unless `result` is a P9999 whose primary label is `labelled`, the
/// text at the start of `anchor` in `source`.
fn assert_labels<T>(source: &str, result: Result<T, Diagnostic>, anchor: &str, labelled: &str) {
    let Err(diagnostic) = result else {
        panic!("the program compiles");
    };
    assert_eq!(diagnostic.code, "P9999");
    let start = source.find(anchor).unwrap();
    assert_eq!(
        (
            diagnostic.primary.location.start,
            diagnostic.primary.location.end
        ),
        (start, start + labelled.len()),
        "{:?}",
        &source[diagnostic.primary.location.start..diagnostic.primary.location.end]
    );
    assert!(
        diagnostic.source_file.is_some(),
        "the compiler location is still recorded for the telemetry dashboards"
    );
}
