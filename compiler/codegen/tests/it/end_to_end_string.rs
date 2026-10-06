//! End-to-end integration tests for STRING initial values.

use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_compile, Snapshot};
use ironplc_container::debug_section::iec_type_tag;

#[test]
fn end_to_end_when_string_initial_value_then_variable_initialized() {
    let source = "
PROGRAM main
  VAR
    x : STRING := 'hello';
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("x"), "hello");
}

#[test]
fn end_to_end_when_string_no_initial_value_then_empty() {
    let source = "
PROGRAM main
  VAR
    x : STRING;
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("x"), "");
}

#[test]
fn end_to_end_when_string_without_length_assigned_longer_value_then_holds_254_characters() {
    let source = format!(
        "
PROGRAM main
  VAR
    x : STRING;
  END_VAR
  x := '{value}';
END_PROGRAM
",
        value = "a".repeat(300),
    );
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());

    assert_eq!(snapshot.read("x"), "a".repeat(254));
}

#[test]
fn end_to_end_when_string_with_length_then_holds_that_many_characters() {
    let source = "
PROGRAM main
  VAR
    x : STRING[10] := 'hi';
    y : STRING[10];
  END_VAR
  y := 'abcdefghijklmnop';
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("x"), "hi");
    assert_eq!(snapshot.read("y"), "abcdefghij");
}

#[test]
fn end_to_end_when_two_string_variables_then_both_initialized() {
    let source = "
PROGRAM main
  VAR
    a : STRING := 'foo';
    b : STRING := 'bar';
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("a"), "foo");
    assert_eq!(snapshot.read("b"), "bar");
}

#[test]
fn end_to_end_when_string_and_int_then_both_work() {
    let source = "
PROGRAM main
  VAR
    x : DINT := 42;
    s : STRING := 'test';
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read_as::<i32>("x"), 42);
    assert_eq!(snapshot.read("s"), "test");
}

#[test]
fn end_to_end_when_string_empty_literal_then_cur_length_zero() {
    let source = "
PROGRAM main
  VAR
    x : STRING := '';
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("x"), "");
}

#[test]
fn end_to_end_when_function_returns_string_with_length_then_executes() {
    // Verify that FUNCTION : STRING[255] compiles and executes through the
    // full pipeline (parse -> analyze -> codegen -> VM) without errors.
    let source = "
FUNCTION my_func : STRING[255]
  VAR_INPUT
    x : INT;
  END_VAR
  my_func := 'hello';
END_FUNCTION

PROGRAM main
  VAR
    result : STRING;
  END_VAR
  result := my_func(1);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "hello");
}

#[test]
fn end_to_end_when_user_function_returns_string_input_then_correct() {
    // Verify that a function accepting STRING[80] and returning STRING[80]
    // can copy the input to the return value via variable assignment.
    let source = "
FUNCTION MY_FUNC : STRING[80]
VAR_INPUT
    str : STRING[80];
END_VAR
    MY_FUNC := str;
END_FUNCTION

PROGRAM main
VAR
    result : STRING[80];
END_VAR
    result := MY_FUNC(str := 'Hello');
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("result"), "Hello");
}

#[test]
fn end_to_end_when_string_declared_then_debug_tag_is_string() {
    let source = "
PROGRAM main
  VAR
    x : STRING;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let var = debug.var_names.iter().find(|v| v.name == "x").unwrap();
    assert_eq!(var.type_name, "STRING");
    assert_eq!(var.iec_type_tag, iec_type_tag::STRING);
}

#[test]
fn end_to_end_when_wstring_declared_then_debug_tag_is_wstring() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let var = debug.var_names.iter().find(|v| v.name == "x").unwrap();
    assert_eq!(var.type_name, "WSTRING");
    assert_eq!(var.iec_type_tag, iec_type_tag::WSTRING);
}
