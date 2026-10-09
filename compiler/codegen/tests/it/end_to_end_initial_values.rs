//! End-to-end tests of the values variables start with: the value the
//! analyzer resolves for each declaration, stored by codegen.
//!
//! Each test is a program whose variables, fields or elements start at a
//! value that only the declared type, not the declaration, states. See
//! `specs/design/initial-values.md`.

use ironplc_dsl::common::{InitialValueAssignmentKind, LibraryElementKind};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;

use crate::common::{compile_analyzed, parse, Snapshot};

const TYPES: &str = "
TYPE
  MYINT : INT := 7;
  RNG : INT(1..10) := 5;
  R : DINT(10..100) := 50;
  R2 : DINT(10..100);
  COLOR : (RED, GREEN, BLUE) := GREEN;
  P : STRUCT x : R2; y : INT := 3; END_STRUCT;
  S : STRUCT
    a : INT := 5;
    b : MYINT;
    r : RNG;
    name : STRING[10] := 'def';
    arr : ARRAY[1..3] OF INT := [1, 2, 3];
  END_STRUCT;
END_TYPE
";

/// Runs `body`, a program and any POUs it uses, after the shared types.
fn run(body: &str) -> Snapshot {
    Snapshot::run(&format!("{TYPES}\n{body}"), &CompilerOptions::default())
}

/// Runs a program that copies `expression` into `out` on its first scan,
/// with `declarations` in its `VAR` block.
fn read_after_init(declarations: &str, expression: &str) -> Snapshot {
    run(&format!(
        "PROGRAM main\nVAR {declarations} END_VAR\nout := {expression};\nEND_PROGRAM"
    ))
}

#[spec_test(REQ_IV_codegen_001)]
fn compile_when_initializer_has_no_value_then_internal_error() {
    let (mut library, context) = parse(
        "PROGRAM main VAR x : INT := 5; END_VAR END_PROGRAM",
        &CompilerOptions::default(),
    );
    for element in &mut library.elements {
        if let LibraryElementKind::ProgramDeclaration(program) = element {
            if let InitialValueAssignmentKind::Simple(simple) =
                &mut program.variables[0].initializer
            {
                simple.initial_value = None;
            }
        }
    }

    let result = compile_analyzed(&library, &context, &CompilerOptions::default());

    assert_eq!(result.unwrap_err().code, "P9998");
}

// --- Structures ---

#[spec_test(REQ_IV_codegen_010)]
fn end_to_end_when_structure_field_declares_default_then_field_starts_at_it() {
    let snapshot = read_after_init("s : S; out : INT;", "s.a");

    assert_eq!(snapshot.read("out"), 5);
}

#[spec_test(REQ_IV_codegen_011)]
fn end_to_end_when_structure_field_of_alias_then_starts_at_alias_default() {
    let snapshot = read_after_init("s : S; out : INT;", "s.b");

    assert_eq!(snapshot.read("out"), 7);
}

#[spec_test(REQ_IV_codegen_012)]
fn end_to_end_when_structure_field_of_subrange_then_starts_at_its_default() {
    let snapshot = read_after_init("s : S; out : INT;", "s.r");

    assert_eq!(snapshot.read("out"), 5);
}

#[spec_test(REQ_IV_codegen_013)]
fn end_to_end_when_structure_string_field_then_default_or_initializer_stored() {
    let snapshot = run("
PROGRAM main
VAR s : S; t : S := (name := 'abc'); sn : STRING[10]; tn : STRING[10]; END_VAR
sn := s.name;
tn := t.name;
END_PROGRAM");

    assert_eq!(snapshot.read("sn"), "def");
    assert_eq!(snapshot.read("tn"), "abc");
}

#[spec_test(REQ_IV_codegen_014)]
fn end_to_end_when_structure_array_field_then_default_or_initializer_stored() {
    let snapshot = run("
PROGRAM main
VAR s : S; t : S := (arr := [4, 5, 6]); sa : INT; ta : INT; END_VAR
sa := s.arr[2];
ta := t.arr[2];
END_PROGRAM");

    assert_eq!(snapshot.read("sa"), 2);
    assert_eq!(snapshot.read("ta"), 5);
}

// --- Scalars ---

#[spec_test(REQ_IV_codegen_020)]
fn end_to_end_when_program_variable_of_alias_with_default_then_starts_at_default() {
    let snapshot = run("PROGRAM main VAR m : MYINT; END_VAR END_PROGRAM");

    assert_eq!(snapshot.read("m"), 7);
}

#[spec_test(REQ_IV_codegen_021)]
fn end_to_end_when_subrange_declares_default_then_variable_starts_at_it() {
    let snapshot = run("PROGRAM main VAR r : R; END_VAR END_PROGRAM");

    assert_eq!(snapshot.read("r"), 50);
}

#[spec_test(REQ_IV_codegen_022)]
fn end_to_end_when_subrange_in_program_function_and_block_then_lower_bound() {
    let snapshot = run("
FUNCTION g : DINT
VAR_INPUT a : DINT; END_VAR
VAR r : R2; END_VAR
g := r;
END_FUNCTION
FUNCTION_BLOCK fb
VAR_OUTPUT o : R2; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR p : R2; f : fb; local : DINT; out : DINT; END_VAR
local := g(a := 0);
f();
out := f.o;
END_PROGRAM");

    assert_eq!(snapshot.read("p"), 10);
    assert_eq!(snapshot.read("local"), 10);
    assert_eq!(snapshot.read("out"), 10);
}

#[spec_test(REQ_IV_codegen_023)]
fn end_to_end_when_function_var_temp_then_reset_on_every_call() {
    let snapshot = run("
FUNCTION f : DINT
VAR_INPUT a : DINT; END_VAR
VAR_TEMP t : DINT := 100; END_VAR
t := t + a;
f := t;
END_FUNCTION
PROGRAM main
VAR first : DINT; second : DINT; END_VAR
first := f(a := 1);
second := f(a := 1);
END_PROGRAM");

    assert_eq!(snapshot.read("first"), 101);
    assert_eq!(snapshot.read("second"), 101);
}

#[spec_test(REQ_IV_codegen_024)]
fn end_to_end_when_function_returning_enumeration_assigns_nothing_then_default() {
    let snapshot = run("
FUNCTION fe : COLOR
VAR_INPUT a : INT; END_VAR
VAR z : INT; END_VAR
z := a;
END_FUNCTION
PROGRAM main
VAR c : COLOR; is_green : BOOL; END_VAR
c := fe(a := 1);
is_green := c = GREEN;
END_PROGRAM");

    assert_eq!(snapshot.read("is_green"), true);
}

#[spec_test(REQ_IV_codegen_025)]
fn end_to_end_when_function_returning_subrange_assigns_nothing_then_lower_bound() {
    let snapshot = run("
FUNCTION fr : R2
VAR_INPUT a : INT; END_VAR
VAR z : INT; END_VAR
z := a;
END_FUNCTION
PROGRAM main
VAR r : DINT; END_VAR
r := fr(a := 1);
END_PROGRAM");

    assert_eq!(snapshot.read("r"), 10);
}

// --- Arrays ---

#[spec_test(REQ_IV_codegen_030)]
fn end_to_end_when_array_of_string_with_empty_repetition_then_elements_empty() {
    let snapshot = run("
PROGRAM main
VAR s : ARRAY[1..3] OF STRING[5] := [1('ab'), 2()]; first : STRING[5]; last : STRING[5]; END_VAR
first := s[1];
last := s[3];
END_PROGRAM");

    assert_eq!(snapshot.read("first"), "ab");
    assert_eq!(snapshot.read("last"), "");
}

#[spec_test(REQ_IV_codegen_031)]
fn end_to_end_when_array_partially_initialized_then_rest_at_element_default() {
    let snapshot = run("
PROGRAM main
VAR a : ARRAY[1..4] OF MYINT := [1, 2(5)]; second : INT; last : INT; END_VAR
second := a[2];
last := a[4];
END_PROGRAM");

    assert_eq!(snapshot.read("second"), 5);
    assert_eq!(snapshot.read("last"), 7);
}

#[spec_test(REQ_IV_codegen_032)]
fn end_to_end_when_array_of_structures_then_every_element_at_structure_default() {
    let snapshot = run("
PROGRAM main
VAR a : ARRAY[1..2] OF P; x : DINT; y : INT; END_VAR
x := a[1].x;
y := a[2].y;
END_PROGRAM");

    assert_eq!(snapshot.read("x"), 10);
    assert_eq!(snapshot.read("y"), 3);
}

#[spec_test(REQ_IV_codegen_033)]
fn end_to_end_when_array_has_more_values_than_elements_then_check_error() {
    let (_, context) = parse(
        "PROGRAM main VAR a : ARRAY[1..3] OF DINT := [1, 2, 3, 4]; END_VAR END_PROGRAM",
        &CompilerOptions::default(),
    );
    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();

    assert_eq!(codes, vec![Problem::ArrayInitializerTooManyValues.code()]);
}

// --- Function block instances ---

#[spec_test(REQ_IV_codegen_040)]
fn end_to_end_when_block_field_declares_default_then_instance_starts_at_it() {
    let snapshot = run("
FUNCTION_BLOCK counter
VAR_INPUT inc : INT := 2; END_VAR
VAR_OUTPUT count : INT := 10; END_VAR
count := count + inc;
END_FUNCTION_BLOCK
PROGRAM main
VAR c : counter; d : counter := (inc := 5); a : INT; b : INT; END_VAR
c();
d();
a := c.count;
b := d.count;
END_PROGRAM");

    assert_eq!(snapshot.read("a"), 12);
    assert_eq!(snapshot.read("b"), 15);
}

#[spec_test(REQ_IV_codegen_041)]
fn end_to_end_when_block_member_initialized_by_expression_then_evaluated_at_initialization() {
    let options = CompilerOptions {
        allow_struct_initializer_expressions: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(
        "
FUNCTION_BLOCK fb
VAR_INPUT limit : INT; END_VAR
VAR_OUTPUT o : INT; END_VAR
o := limit;
END_FUNCTION_BLOCK
PROGRAM main
VAR base : INT := 4; b : fb := (limit := base + 1); out : INT; END_VAR
b();
out := b.o;
END_PROGRAM",
        &options,
    );

    assert_eq!(snapshot.read("out"), 5);
}
