//! End-to-end tests for `VAR_TEMP` (#1857): a temporary variable starts from
//! its initial value on every execution of its POU, in a PROGRAM (every
//! scan) and in a FUNCTION (every call).

use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run_rounds;

#[test]
fn end_to_end_when_program_var_temp_then_reinitialized_every_scan() {
    // q accumulates t, and t is 5 + 1 on every scan: 6, then 12. A t kept
    // across scans would give 6, then 13.
    let source = "
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP t : INT := 5; END_VAR
  t := t + 1;
  q := q + t;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 6);
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 12);
    });
}

#[test]
fn end_to_end_when_program_var_temp_first_section_then_accepted() {
    let source = "
PROGRAM main
  VAR_TEMP t : DINT; END_VAR
  VAR q : DINT; END_VAR
  t := t + 7;
  q := q + t;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 14);
    });
}

#[test]
fn end_to_end_when_function_var_temp_then_reinitialized_every_call() {
    let source = "
FUNCTION f : INT
  VAR_TEMP t : INT := 1; END_VAR
  t := t + 1;
  f := t;
END_FUNCTION
PROGRAM main
  VAR a : INT; b : INT; END_VAR
  a := f();
  b := f();
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 2);
        assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 2);
    });
}

#[test]
fn end_to_end_when_program_var_temp_structure_then_reinitialized_every_scan() {
    let source = "
TYPE P : STRUCT x : INT; END_STRUCT; END_TYPE
PROGRAM main
  VAR q : INT; END_VAR
  VAR_TEMP p : P := (x := 5); END_VAR
  p.x := p.x + 1;
  q := q + p.x;
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 12);
    });
}
