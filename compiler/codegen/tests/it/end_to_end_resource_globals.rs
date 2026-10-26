//! End-to-end integration tests for global variables declared in a RESOURCE.
//!
//! A `VAR_GLOBAL` inside a `RESOURCE` is visible to the programs of that
//! resource (IEC 61131-3, 2.7.1). These tests compile such a program and run
//! it in the VM, reading and writing the global through `VAR_EXTERNAL`.

use ironplc_dsl::common::{Library, LibraryElementKind};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

use crate::common::{compile_analyzed, run_scans};

e2e_i32!(
    end_to_end_when_resource_global_written_via_external_then_value_updated,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count : INT;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    count : INT;
  END_VAR
  count := count + 1;
END_PROGRAM
",
    &[("count", 1)],
);

e2e_i32!(
    end_to_end_when_resource_global_has_initial_value_then_external_reads_value,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      limit : DINT := 250;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    limit : DINT;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  result := limit;
END_PROGRAM
",
    &[("limit", 250), ("result", 250)],
);

e2e_i32!(
    end_to_end_when_configuration_and_resource_globals_then_both_readable,
    "
CONFIGURATION config
  VAR_GLOBAL
    base : INT := 100;
  END_VAR
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      offset : INT := 7;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    base : INT;
    offset : INT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  result := base + offset;
END_PROGRAM
",
    &[("base", 100), ("offset", 7), ("result", 107)],
);

e2e_i32!(
    end_to_end_when_resource_global_array_then_elements_readable_and_writable,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      readings : ARRAY[1..3] OF INT := [10, 20, 30];
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    readings : ARRAY[1..3] OF INT;
  END_VAR
  VAR
    total : INT;
  END_VAR
  readings[2] := readings[2] + 5;
  total := readings[1] + readings[2] + readings[3];
END_PROGRAM
",
    &[("total", 65)],
);

e2e_i32!(
    end_to_end_when_resource_global_struct_then_fields_readable_and_writable,
    "
TYPE POINT :
  STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      point : POINT;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    point : POINT;
  END_VAR
  VAR
    sum : DINT;
  END_VAR
  point.x := 3;
  point.y := 4;
  sum := point.x + point.y;
END_PROGRAM
",
    &[("sum", 7)],
);

#[test]
fn end_to_end_when_resource_global_incremented_over_scans_then_value_persists() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      count : DINT := 10;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        for (scan, expected) in (0..3).zip(11..) {
            session.scan(scan * 100_000).unwrap();
            assert_eq!(session.read("count"), expected);
        }
    });
}

e2e_i32!(
    end_to_end_when_resource_global_wstring_then_length_readable,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      label : WSTRING := \"abc\";
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    label : WSTRING;
  END_VAR
  VAR
    length : INT;
  END_VAR
  length := LEN(label);
END_PROGRAM
",
    &[("length", 3)],
);

e2e_i32!(
    end_to_end_when_resource_global_array_of_strings_then_elements_writable,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      names : ARRAY[1..2] OF STRING;
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    names : ARRAY[1..2] OF STRING;
  END_VAR
  VAR
    length : INT;
  END_VAR
  names[2] := 'xyz';
  length := LEN(names[1]) + LEN(names[2]);
END_PROGRAM
",
    &[("length", 3)],
);

e2e_i32!(
    end_to_end_when_resource_global_two_dimensional_array_then_initial_elements_readable,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      grid : ARRAY[1..2, 1..2] OF INT := [1, 2, 3, 4];
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    grid : ARRAY[1..2, 1..2] OF INT;
  END_VAR
  VAR
    corner : INT;
  END_VAR
  corner := grid[2, 1];
END_PROGRAM
",
    &[("corner", 3)],
);

e2e_i32!(
    end_to_end_when_resource_global_array_of_structs_then_fields_writable,
    "
TYPE POINT :
  STRUCT
    x : INT;
  END_STRUCT;
END_TYPE

CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      points : ARRAY[1..2] OF POINT;
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    points : ARRAY[1..2] OF POINT;
  END_VAR
  VAR
    result : INT;
  END_VAR
  points[2].x := 5;
  result := points[1].x + points[2].x;
END_PROGRAM
",
    &[("result", 5)],
);

e2e_i32!(
    end_to_end_when_resource_global_struct_with_string_field_then_field_writable,
    "
TYPE TAG :
  STRUCT
    name : STRING;
    id : INT;
  END_STRUCT;
END_TYPE

CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      tag : TAG;
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    tag : TAG;
  END_VAR
  VAR
    length : INT;
  END_VAR
  tag.name := 'hi';
  length := LEN(tag.name);
END_PROGRAM
",
    &[("length", 2)],
);

e2e_i32!(
    end_to_end_when_resource_global_function_block_then_callable_via_external,
    "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      timer : TON;
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    timer : TON;
  END_VAR
  VAR
    running : INT;
  END_VAR
  timer(IN := TRUE, PT := T#1s);
  IF NOT timer.Q THEN
    running := 1;
  END_IF;
END_PROGRAM
",
    &[("running", 1)],
);

#[test]
fn end_to_end_when_resource_global_string_appended_over_scans_then_value_persists() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      text : STRING := 'abc';
    END_VAR
    PROGRAM main_instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    text : STRING;
  END_VAR
  VAR
    length : INT;
    matched : BOOL;
  END_VAR
  length := LEN(text);
  matched := text = 'abcd';
  text := CONCAT(text, 'd');
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        session.scan(0).unwrap();
        assert_eq!(session.read("length"), 3);
        assert_eq!(session.read("matched"), false);
        assert_eq!(session.read("text"), "abcd");
        session.scan(1).unwrap();
        assert_eq!(session.read("length"), 4);
        assert_eq!(session.read("matched"), true);
        assert_eq!(session.read("text"), "abcdd");
    });
}

/// The configuration `library` declares.
fn configuration_mut(library: &mut Library) -> &mut ConfigurationDeclaration {
    library
        .elements
        .iter_mut()
        .find_map(|element| match element {
            LibraryElementKind::ConfigurationDeclaration(config) => Some(config),
            LibraryElementKind::DataTypeDeclaration(_)
            | LibraryElementKind::FunctionDeclaration(_)
            | LibraryElementKind::FunctionBlockDeclaration(_)
            | LibraryElementKind::ProgramDeclaration(_)
            | LibraryElementKind::GlobalVarDeclarations(_)
            | LibraryElementKind::InterfaceDeclaration(_) => None,
        })
        .unwrap()
}

#[test]
fn compile_when_other_resource_declares_globals_then_they_get_no_storage() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      mine : INT;
    END_VAR
    PROGRAM instance1 : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    mine : INT;
  END_VAR
  mine := 1;
END_PROGRAM
";
    // The structured text grammar takes one RESOURCE per configuration; the
    // PLCopen XML front end gives a configuration as many as it declares. So
    // the second resource is parsed on its own and added to the first. The
    // grammar also needs a program in it, which is removed so that the
    // second resource runs nothing.
    let other = "
CONFIGURATION other
  RESOURCE resource2 ON PLC
    VAR_GLOBAL
      theirs : INT;
    END_VAR
    PROGRAM unused : main;
  END_RESOURCE
END_CONFIGURATION
";
    let options = CompilerOptions::default();
    let mut library = parse_program(source, &FileId::default(), &options).unwrap();
    let mut other = parse_program(other, &FileId::default(), &options).unwrap();
    let mut resource2 = configuration_mut(&mut other).resource_decl.remove(0);
    resource2.programs.clear();
    configuration_mut(&mut library)
        .resource_decl
        .push(resource2);
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let container = compile_analyzed(&library, &context, &options).unwrap();

    let names: Vec<&str> = container
        .debug_section
        .as_ref()
        .unwrap()
        .var_names
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert!(names.contains(&"mine"));
    assert!(!names.contains(&"theirs"));
    assert_eq!(container.header.num_variables, 1);
}
