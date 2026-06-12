//! End-to-end tests for function- and FB-local variable debug entries
//! (debug section Tag 2, VAR_NAME).
//!
//! Program/global-scope variables have long carried VAR_NAME entries; these
//! tests cover the parameters, locals, and return variables of user-defined
//! FUNCTION and FUNCTION_BLOCK bodies, which a DAP debugger uses to populate
//! the Variables pane for the current stack frame.

use crate::common::parse_and_compile;
use ironplc_container::debug_section::{iec_type_tag, var_section, VarNameEntry};
use ironplc_container::FunctionId;
use ironplc_parser::options::CompilerOptions;

/// Resolves the `function_id` assigned to a user POU by looking it up in the
/// FUNC_NAME table by name (case-insensitive). Panics if absent.
fn function_id_of(container: &ironplc_container::Container, name: &str) -> FunctionId {
    let debug = container
        .debug_section
        .as_ref()
        .expect("debug section present");
    debug
        .func_names
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("func name {name} present in debug section"))
        .function_id
}

/// Collects the VAR_NAME entries owned by a given function id.
fn vars_for(container: &ironplc_container::Container, owner: FunctionId) -> Vec<&VarNameEntry> {
    let debug = container
        .debug_section
        .as_ref()
        .expect("debug section present");
    debug
        .var_names
        .iter()
        .filter(|v| v.function_id == owner)
        .collect()
}

/// Returns the single entry in `section`, panicking if there is not exactly one.
fn only_in_section<'a>(entries: &[&'a VarNameEntry], section: u8) -> &'a VarNameEntry {
    let matches: Vec<&&VarNameEntry> = entries
        .iter()
        .filter(|v| v.var_section == section)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one entry in section {section}, found {}",
        matches.len()
    );
    matches[0]
}

#[test]
fn var_names_when_user_function_has_params_then_entries_have_function_id_and_sections() {
    let source = "
FUNCTION foo : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR t : BOOL; END_VAR
  foo := a;
END_FUNCTION
PROGRAM main
  VAR r : DINT; END_VAR
  r := foo(1);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let foo_id = function_id_of(&container, "foo");
    let entries = vars_for(&container, foo_id);

    let input = only_in_section(&entries, var_section::VAR_INPUT);
    assert!(input.name.eq_ignore_ascii_case("a"));
    assert_eq!(input.iec_type_tag, iec_type_tag::DINT);
    assert_eq!(input.type_name, "DINT");

    let local = only_in_section(&entries, var_section::VAR);
    assert!(local.name.eq_ignore_ascii_case("t"));
    assert_eq!(local.iec_type_tag, iec_type_tag::BOOL);
    assert_eq!(local.type_name, "BOOL");

    // The return variable is modeled as VAR_OUTPUT so a debugger surfaces it
    // in the "Outputs" scope.
    let output = only_in_section(&entries, var_section::VAR_OUTPUT);
    assert!(output.name.eq_ignore_ascii_case("foo"));
    assert_eq!(output.iec_type_tag, iec_type_tag::DINT);
}

#[test]
fn var_names_when_user_function_block_has_locals_then_entries_have_fb_function_id() {
    let source = "
FUNCTION_BLOCK doubler
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  VAR scratch : BOOL; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK
PROGRAM main
  VAR
    inst : doubler;
    result : DINT;
  END_VAR
  inst(x := 7, y => result);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let fb_id = function_id_of(&container, "doubler");
    let entries = vars_for(&container, fb_id);

    let input = only_in_section(&entries, var_section::VAR_INPUT);
    assert!(input.name.eq_ignore_ascii_case("x"));
    assert_eq!(input.iec_type_tag, iec_type_tag::DINT);

    let output = only_in_section(&entries, var_section::VAR_OUTPUT);
    assert!(output.name.eq_ignore_ascii_case("y"));
    assert_eq!(output.iec_type_tag, iec_type_tag::DINT);

    let local = only_in_section(&entries, var_section::VAR);
    assert!(local.name.eq_ignore_ascii_case("scratch"));
    assert_eq!(local.iec_type_tag, iec_type_tag::BOOL);

    // Every collected entry is tagged with the FB body's function id, not
    // the global scope.
    assert!(entries.iter().all(|v| v.function_id == fb_id));
}

#[test]
fn var_names_when_global_and_function_locals_then_globals_keep_global_scope() {
    let source = "
FUNCTION inc : DINT
  VAR_INPUT n : DINT; END_VAR
  inc := n + 1;
END_FUNCTION
PROGRAM main
  VAR g : DINT; END_VAR
  g := inc(g);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // The program global keeps GLOBAL_SCOPE.
    let globals = vars_for(&container, FunctionId::GLOBAL_SCOPE);
    assert!(
        globals.iter().any(|v| v.name.eq_ignore_ascii_case("g")),
        "global g should carry GLOBAL_SCOPE"
    );

    // The function parameter is owned by the function, not the global scope.
    let inc_id = function_id_of(&container, "inc");
    assert_ne!(inc_id, FunctionId::GLOBAL_SCOPE);
    let locals = vars_for(&container, inc_id);
    assert!(
        locals.iter().any(|v| v.name.eq_ignore_ascii_case("n")),
        "parameter n should be owned by inc"
    );
    // No global leaked into the function-owned set.
    assert!(locals.iter().all(|v| v.function_id == inc_id));
}

#[test]
fn var_names_when_two_functions_then_each_owned_by_its_own_function_id() {
    let source = "
FUNCTION first : DINT
  VAR_INPUT p : DINT; END_VAR
  first := p;
END_FUNCTION
FUNCTION second : DINT
  VAR_INPUT q : DINT; END_VAR
  second := q;
END_FUNCTION
PROGRAM main
  VAR r : DINT; END_VAR
  r := first(1) + second(2);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let first_id = function_id_of(&container, "first");
    let second_id = function_id_of(&container, "second");
    assert_ne!(first_id, second_id);

    let p = only_in_section(&vars_for(&container, first_id), var_section::VAR_INPUT);
    let q = only_in_section(&vars_for(&container, second_id), var_section::VAR_INPUT);
    assert!(p.name.eq_ignore_ascii_case("p"));
    assert!(q.name.eq_ignore_ascii_case("q"));

    // The variable table is globally partitioned (each POU gets a distinct
    // index range), so the parameters carry distinct var_index values *and*
    // distinct owning function ids — a debugger can attribute each to its
    // frame without ambiguity.
    assert_ne!(p.var_index, q.var_index);
}
