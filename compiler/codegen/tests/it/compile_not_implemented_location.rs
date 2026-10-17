//! A P9999 raised by codegen names the construct the compiler cannot handle
//! yet, so the CLI can show the offending line rather than `┌─ :1:1` (issue
//! #1734). Each case is a program that `check` accepts and `compile` refuses;
//! the assertion is on where the primary label lands.

use crate::common::{compile_by_route, parse, try_parse_and_compile};
use ironplc_codegen::Route;
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

    assert_labels(source, result, anchor, labelled);
}

/// The route that reads the AST does not read a function's string output.
/// The lowered route does (REQ-LOW-lowering-172), so the program is compiled
/// by the AST route alone.
#[test]
fn compile_when_string_output_of_function_on_ast_route_then_primary_label_names_the_output() {
    let source = "
FUNCTION F : DINT
VAR_INPUT x : DINT; END_VAR
VAR_OUTPUT name : STRING; END_VAR
    name := 'motor';
    F := x;
END_FUNCTION

PROGRAM main
VAR
    r : DINT;
    s : STRING;
END_VAR
    r := F(x := 1, name => s);
END_PROGRAM
";
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let result = compile_by_route(&library, &context, &options, Route::Ast);

    assert_labels(source, result, "name => s", "name");
}

/// The route that reads the AST copies a whole structure only from a whole
/// variable or a function result. Analysis accepts a source of the target's
/// type that is an element or a field too, so the AST route reports it as
/// not implemented, at the source.
#[rstest]
#[case::element_source("r := ps[2];", "ps[2];", "ps")]
#[case::field_source("r := o.inner;", "o.inner;", "o.inner")]
fn compile_when_copy_from_element_or_field_on_ast_route_then_primary_label_names_the_source(
    #[case] statement: &str,
    #[case] anchor: &str,
    #[case] labelled: &str,
) {
    let source = format!(
        "
TYPE
  P : STRUCT a : DINT; END_STRUCT;
  Outer : STRUCT inner : P; END_STRUCT;
END_TYPE

PROGRAM main
VAR
    ps : ARRAY[1..2] OF P;
    o : Outer;
    r : P;
END_VAR
    {statement}
END_PROGRAM
"
    );
    let options = CompilerOptions::default();
    let (library, context) = parse(&source, &options);
    let result = compile_by_route(&library, &context, &options, Route::Ast);

    assert_labels(&source, result, anchor, labelled);
}

/// Fails unless `result` is a P9999 whose primary label is `labelled`, the
/// text at the start of `anchor` in `source`.
fn assert_labels<T>(source: &str, result: Result<T, Diagnostic>, anchor: &str, labelled: &str) {
    let Err(diagnostic) = result else {
        panic!("the program compiles");
    };
    assert_eq!(diagnostic.code, "P9999");
    let start = source.find(anchor).unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + labelled.len());
    assert!(
        diagnostic.source_file.is_some(),
        "the compiler location is still recorded for the telemetry dashboards"
    );
}
