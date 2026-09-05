//! The VM runs one program instance, so codegen refuses a second `PROGRAM`
//! declaration or a second program instance instead of silently keeping one
//! of them (issue #1588). The diagnostic points at every program so the reader
//! can see which ones the compiler would have dropped.

use crate::common::try_parse_and_compile;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;

/// A library whose program declarations are `PROGRAM prog_a`, `PROGRAM prog_b`
/// and so on, in source order, followed by a configuration that instantiates
/// each of them on the same task.
fn programs(names: &[&str]) -> String {
    let declarations: String = names
        .iter()
        .map(|name| {
            format!("PROGRAM {name}\nVAR x : DINT; END_VAR\n  x := x + 1;\nEND_PROGRAM\n\n")
        })
        .collect();
    let instances: String = names
        .iter()
        .map(|name| format!("    PROGRAM inst_{name} WITH t : {name};\n"))
        .collect();
    format!(
        "{declarations}CONFIGURATION config
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
{instances}  END_RESOURCE
END_CONFIGURATION
"
    )
}

fn compile_err(source: &str) -> Diagnostic {
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(result.is_err(), "expected the compile to be rejected");
    result.unwrap_err()
}

/// Byte offset of `needle` in `source`, so a test can assert that a label
/// lands on a specific declaration rather than on whichever one toposort
/// happened to put first.
fn offset_of(source: &str, needle: &str) -> usize {
    source.find(needle).expect("needle is in the source") + "PROGRAM ".len()
}

#[test]
fn compile_when_two_programs_then_p9999_on_second_program() {
    let source = programs(&["prog_a", "prog_b"]);

    let diagnostic = compile_err(&source);

    assert_eq!(diagnostic.code, "P9999");
    assert_eq!(
        diagnostic.primary.location.start,
        offset_of(&source, "PROGRAM prog_b")
    );
    assert!(diagnostic
        .primary
        .message
        .contains("2nd PROGRAM declaration"));
    assert_eq!(diagnostic.secondary.len(), 1);
    assert_eq!(
        diagnostic.secondary[0].location.start,
        offset_of(&source, "PROGRAM prog_a")
    );
    assert_eq!(diagnostic.secondary[0].message, "1st PROGRAM declaration");
}

#[test]
fn compile_when_two_programs_then_help_names_tracking_issue() {
    let source = programs(&["prog_a", "prog_b"]);

    let diagnostic = compile_err(&source);

    assert!(diagnostic
        .help()
        .iter()
        .any(|help| help.contains("issues/1613")));
}

#[test]
fn compile_when_three_programs_then_labels_first_and_third_as_secondary() {
    let source = programs(&["prog_a", "prog_b", "prog_c"]);

    let diagnostic = compile_err(&source);

    assert_eq!(diagnostic.code, "P9999");
    assert_eq!(
        diagnostic.primary.location.start,
        offset_of(&source, "PROGRAM prog_b")
    );
    let secondary: Vec<(usize, &str)> = diagnostic
        .secondary
        .iter()
        .map(|label| (label.location.start, label.message.as_str()))
        .collect();
    assert_eq!(
        secondary,
        vec![
            (
                offset_of(&source, "PROGRAM prog_a"),
                "1st PROGRAM declaration"
            ),
            (
                offset_of(&source, "PROGRAM prog_c"),
                "3rd PROGRAM declaration"
            ),
        ]
    );
}

#[test]
fn compile_when_two_instances_of_one_program_then_p9999_on_second_instance() {
    let source = "
PROGRAM main
VAR x : DINT; END_VAR
  x := x + 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM first WITH t : main;
    PROGRAM second WITH t : main;
  END_RESOURCE
END_CONFIGURATION
";

    let diagnostic = compile_err(source);

    assert_eq!(diagnostic.code, "P9999");
    assert_eq!(
        diagnostic.primary.location.start,
        offset_of(source, "PROGRAM second")
    );
    assert!(diagnostic.primary.message.contains("2nd program instance"));
    assert_eq!(diagnostic.secondary.len(), 1);
    assert_eq!(diagnostic.secondary[0].message, "1st program instance");
}

#[test]
fn compile_when_one_program_and_one_instance_then_ok() {
    let source = programs(&["prog_a"]);

    let result = try_parse_and_compile(&source, &CompilerOptions::default());

    assert!(result.is_ok());
}
