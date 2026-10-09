//! Tests of the initializers the analyzer completes on each declaration.
//!
//! A completed initializer is shown as text by [`shown`]: the value slot of
//! the initializer, with `~` before every part that has a synthesized span,
//! that is, every part the program did not write at the declaration.

use ironplc_dsl::common::*;
use ironplc_dsl::core::{FileId, Id, Located};
use ironplc_dsl::textual::ExprKind;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

use ironplc_problems::Problem;

use crate::stages::analyze;
use crate::test_helpers::{edition3_options, fb_inheritance_options};
use rstest::rstest;
use spec_test_macro::spec_test;

/// Parses and analyzes `program`, which must analyze without problems.
fn resolve_with(program: &str, options: &CompilerOptions) -> Library {
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    library
}

fn resolve(program: &str) -> Library {
    resolve_with(program, &CompilerOptions::default())
}

/// The declarations of the POU (or configuration) named `pou`, or the
/// top-level globals when `pou` is empty. A method is `BLOCK.METHOD`.
fn declarations<'a>(library: &'a Library, pou: &str) -> Vec<&'a VarDecl> {
    let mut found = Vec::new();
    for element in &library.elements {
        match element {
            LibraryElementKind::ProgramDeclaration(p) if p.name == Id::from(pou) => {
                found.extend(&p.variables)
            }
            LibraryElementKind::FunctionDeclaration(f) if f.name == Id::from(pou) => {
                found.extend(&f.variables)
            }
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                if fb.name.name == Id::from(pou) {
                    found.extend(&fb.variables);
                }
                for method in &fb.methods {
                    if format!("{}.{}", fb.name.name, method.name).eq_ignore_ascii_case(pou) {
                        found.extend(&method.variables);
                    }
                }
            }
            LibraryElementKind::ConfigurationDeclaration(c) if c.name == Id::from(pou) => {
                found.extend(&c.global_var)
            }
            LibraryElementKind::GlobalVarDeclarations(globals) if pou.is_empty() => {
                found.extend(globals)
            }
            _ => {}
        }
    }
    found
}

/// The declaration of `variable` in `pou`.
fn decl<'a>(library: &'a Library, pou: &str, variable: &str) -> &'a VarDecl {
    declarations(library, pou)
        .into_iter()
        .find(|d| d.identifier.symbolic_id() == Some(&Id::from(variable)))
        .unwrap()
}

/// The starting value of `variable` in `pou`, as [`shown`] shows it.
fn value(library: &Library, pou: &str, variable: &str) -> String {
    shown(&decl(library, pou, variable).initializer)
}

fn function<'a>(library: &'a Library, name: &str) -> &'a FunctionDeclaration {
    library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::FunctionDeclaration(f) if f.name == Id::from(name) => Some(f),
            _ => None,
        })
        .unwrap()
}

/// The value slot of `initializer` as text, `-` when it is empty.
fn shown(initializer: &InitialValueAssignmentKind) -> String {
    use InitialValueAssignmentKind as Kind;
    match initializer {
        Kind::Simple(SimpleInitializer {
            initial_value: Some(constant),
            ..
        }) => constant_text(constant),
        Kind::String(StringInitializer {
            initial_value: Some(literal),
            ..
        }) => constant_text(&ConstantKind::CharacterString(literal.clone())),
        Kind::EnumeratedValues(EnumeratedValuesInitializer {
            initial_value: Some(value),
            ..
        })
        | Kind::EnumeratedType(EnumeratedInitialValueAssignment {
            initial_value: Some(value),
            ..
        }) => enumerated_text(value),
        Kind::Subrange(SubrangeInitialValueAssignment {
            initial_value: Some(value),
            ..
        }) => mark(value.value.span.is_synthesized(), value.to_string()),
        Kind::Reference(ReferenceInitializer {
            initial_value: Some(ReferenceInitialValue::Null(span)),
            ..
        }) => mark(span.is_synthesized(), "NULL".to_owned()),
        Kind::Reference(ReferenceInitializer {
            initial_value: Some(ReferenceInitialValue::Ref(variable)),
            ..
        }) => format!("REF({variable})"),
        Kind::Array(array) if !array.initial_values.is_empty() => {
            elements_text(&array.initial_values)
        }
        Kind::Structure(structure) if !structure.elements_init.is_empty() => {
            members_text(&structure.elements_init)
        }
        Kind::FunctionBlock(block) if !block.init.is_empty() => members_text(&block.init),
        _ => "-".to_owned(),
    }
}

fn mark(synthesized: bool, text: String) -> String {
    if synthesized {
        format!("~{text}")
    } else {
        text
    }
}

fn constant_text(constant: &ConstantKind) -> String {
    let text = match constant {
        ConstantKind::RealLiteral(literal) => format!("{:?}", literal.value),
        ConstantKind::Duration(literal) => {
            format!("T#{}ms", literal.interval.whole_milliseconds())
        }
        ConstantKind::TimeOfDay(literal) => format!("TOD#{}ms", literal.whole_milliseconds()),
        ConstantKind::Date(literal) => format!("D#{}s", literal.seconds_since_epoch()),
        ConstantKind::DateAndTime(literal) => format!("DT#{}s", literal.seconds_since_epoch()),
        other => other.to_string(),
    };
    mark(constant.span().is_synthesized(), text)
}

fn enumerated_text(value: &EnumeratedValue) -> String {
    mark(value.value.span.is_synthesized(), value.value.to_string())
}

fn elements_text(elements: &[ArrayInitialElementKind]) -> String {
    let texts: Vec<String> = elements
        .iter()
        .map(|element| match element {
            ArrayInitialElementKind::Constant(constant) => constant_text(constant),
            ArrayInitialElementKind::EnumValue(value) => enumerated_text(value),
            ArrayInitialElementKind::Structure(members) => members_text(members),
            ArrayInitialElementKind::Expression(expr) => expression_text(&expr.kind),
            ArrayInitialElementKind::Repeated(_) => "repeated".to_owned(),
        })
        .collect();
    format!("[{}]", texts.join(", "))
}

fn members_text(members: &[StructureElementInit]) -> String {
    let texts: Vec<String> = members
        .iter()
        .map(|member| {
            let value = match &member.init {
                StructInitialValueAssignmentKind::Constant(constant) => constant_text(constant),
                StructInitialValueAssignmentKind::EnumeratedValue(value) => enumerated_text(value),
                StructInitialValueAssignmentKind::Array(elements) => elements_text(elements),
                StructInitialValueAssignmentKind::Structure(members) => members_text(members),
                StructInitialValueAssignmentKind::Expression(expr) => expression_text(&expr.kind),
                StructInitialValueAssignmentKind::LateBound(_) => "late".to_owned(),
            };
            let name = mark(member.name.span.is_synthesized(), member.name.to_string());
            format!("{name} := {value}")
        })
        .collect();
    format!("({})", texts.join(", "))
}

fn expression_text(kind: &ExprKind) -> String {
    match kind {
        ExprKind::Null(span) => mark(span.is_synthesized(), "NULL".to_owned()),
        _ => "expr".to_owned(),
    }
}

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

fn program(body: &str) -> String {
    format!("{TYPES}\nPROGRAM main\n{body}\nEND_PROGRAM\n")
}

// --- Completed initializers ---

#[spec_test(REQ_IV_analyzer_001)]
fn apply_when_declarations_then_each_initializer_completed_but_in_out_and_external() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL g : INT := 3; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
FUNCTION f : INT
VAR_IN_OUT io : INT; END_VAR
f := io;
END_FUNCTION
PROGRAM main
VAR_EXTERNAL g : INT; END_VAR
VAR x : INT := 2; y : INT; END_VAR
y := f(io := x);
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(value(&library, "config", "g"), "3");
    assert_eq!(value(&library, "main", "x"), "2");
    assert_eq!(value(&library, "main", "y"), "~0");
    assert_eq!(value(&library, "main", "g"), "-");
    assert_eq!(value(&library, "f", "io"), "-");
}

#[spec_test(REQ_IV_analyzer_002)]
fn apply_when_partial_initializer_then_only_supplied_parts_synthesized() {
    let library = resolve(&program(
        "VAR p : P := (y := 4); a : ARRAY[1..3] OF INT := [1]; END_VAR",
    ));

    assert_eq!(value(&library, "main", "p"), "(~x := ~10, y := 4)");
    assert_eq!(value(&library, "main", "a"), "[1, ~0, ~0]");
}

#[spec_test(REQ_IV_analyzer_003)]
fn apply_when_literal_of_other_form_then_rewritten_as_stored_type_at_same_span() {
    let library = resolve(&program(
        "VAR x : REAL := 2; y : LREAL := 0.5; t : TIME := T#1s500ms; END_VAR",
    ));

    assert_eq!(value(&library, "main", "x"), "2.0");
    assert_eq!(value(&library, "main", "y"), "0.5");
    assert_eq!(value(&library, "main", "t"), "T#1500ms");
    let InitialValueAssignmentKind::Simple(SimpleInitializer {
        initial_value: Some(constant @ ConstantKind::RealLiteral(_)),
        ..
    }) = &decl(&library, "main", "x").initializer
    else {
        panic!("expected a real literal");
    };
    assert!(!constant.span().is_synthesized());
}

#[spec_test(REQ_IV_analyzer_004)]
fn apply_when_function_and_method_then_result_variable_starts_at_type_default() {
    let source = format!(
        "{TYPES}
FUNCTION_BLOCK fb
METHOD speed : R2
VAR m : INT := 6; END_VAR
m := 1;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION f : MYINT
VAR_INPUT a : INT; END_VAR
f := a;
END_FUNCTION
FUNCTION g : STRING[8]
g := 'x';
END_FUNCTION
PROGRAM main
VAR b : fb; x : INT; END_VAR
x := f(a := 1);
END_PROGRAM"
    );
    let library = resolve_with(&source, &fb_inheritance_options());
    let method = library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb) => fb.methods.first(),
            _ => None,
        })
        .unwrap();

    let f = function(&library, "f").result.variable().unwrap();
    assert_eq!(f.identifier.symbolic_id(), Some(&Id::from("f")));
    assert_eq!(shown(&f.initializer), "~7");
    assert_eq!(shown(&method.result.variable().unwrap().initializer), "~10");
    assert_eq!(
        shown(
            &function(&library, "g")
                .result
                .variable()
                .unwrap()
                .initializer
        ),
        "~''"
    );
}

#[spec_test(REQ_IV_analyzer_005)]
fn apply_when_function_local_hides_global_of_other_type_then_own_value() {
    let source = "
CONFIGURATION config
  VAR_GLOBAL v : REAL := 1.5; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
FUNCTION f : INT
VAR v : INT := 4; END_VAR
f := v;
END_FUNCTION
PROGRAM main
VAR_EXTERNAL v : REAL; END_VAR
VAR x : INT; END_VAR
x := f();
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(value(&library, "f", "v"), "4");
    assert_eq!(value(&library, "config", "v"), "1.5");
}

// --- Structures ---

/// The completed initializer of `s : S`, a structure no initializer sets.
fn default_structure() -> String {
    value(&resolve(&program("VAR s : S; END_VAR")), "main", "s")
}

#[spec_test(REQ_IV_analyzer_010)]
fn apply_when_structure_field_declares_initializer_then_field_starts_at_it() {
    let s = default_structure();

    assert!(s.contains("~a := ~5,"), "{s}");
}

#[spec_test(REQ_IV_analyzer_011)]
fn apply_when_structure_field_of_alias_then_starts_at_alias_default() {
    let s = default_structure();

    assert!(s.contains("~b := ~7,"), "{s}");
}

#[spec_test(REQ_IV_analyzer_012)]
fn apply_when_structure_field_of_subrange_with_default_then_starts_at_it() {
    let s = default_structure();

    assert!(s.contains("~r := ~5,"), "{s}");
}

#[spec_test(REQ_IV_analyzer_014)]
fn apply_when_structure_string_and_array_fields_then_declared_defaults() {
    let s = default_structure();

    assert_eq!(
        s,
        "(~a := ~5, ~b := ~7, ~r := ~5, ~name := ~'def', ~arr := [~1, ~2, ~3])"
    );
}

#[spec_test(REQ_IV_analyzer_013)]
fn apply_when_structure_string_field_initialized_then_initializer_value() {
    let library = resolve(&program("VAR t : S := (name := 'abc'); END_VAR"));

    assert_eq!(
        value(&library, "main", "t"),
        "(~a := ~5, ~b := ~7, ~r := ~5, name := 'abc', ~arr := [~1, ~2, ~3])"
    );
}

#[spec_test(REQ_IV_analyzer_015)]
fn apply_when_structure_array_field_initialized_then_initializer_value() {
    let library = resolve(&program("VAR t : S := (arr := [4, 5]); END_VAR"));

    assert_eq!(
        value(&library, "main", "t"),
        "(~a := ~5, ~b := ~7, ~r := ~5, ~name := ~'def', arr := [4, 5, ~0])"
    );
}

#[spec_test(REQ_IV_analyzer_016)]
fn apply_when_structures_nested_in_arrays_in_structures_then_innermost_defaults() {
    let source = "
TYPE
  INNER : STRUCT v : INT := 4; END_STRUCT;
  HOLDER : STRUCT items : ARRAY[1..2] OF INNER; nested : INNER; END_STRUCT;
  OUTER : STRUCT h : HOLDER; END_STRUCT;
END_TYPE
PROGRAM main
VAR o : OUTER; END_VAR
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(
        value(&library, "main", "o"),
        "(~h := (~items := [(~v := ~4), (~v := ~4)], ~nested := (~v := ~4)))"
    );
}

#[spec_test(REQ_IV_analyzer_017)]
fn apply_when_structure_field_subrange_states_value_then_field_starts_at_it() {
    let source = "
TYPE
  T : STRUCT level : INT (0..15) := 3; other : INT (2..15); END_STRUCT;
END_TYPE
PROGRAM main
VAR t : T; END_VAR
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(value(&library, "main", "t"), "(~level := ~3, ~other := ~2)");
}

// --- Scalars ---

#[spec_test(REQ_IV_analyzer_020)]
#[rstest]
#[case::alias_default("m : MYINT;", "m", "~7")]
#[case::explicit_over_alias_default("m : MYINT := 9;", "m", "9")]
#[case::enumeration_default("c : COLOR;", "c", "~GREEN")]
#[case::elementary_zero("n : LINT;", "n", "~0")]
#[case::real_zero("n : LREAL;", "n", "~0.0")]
#[case::bool_false("n : BOOL;", "n", "~FALSE")]
#[case::time_zero("n : LTIME;", "n", "~T#0ms")]
#[case::date_epoch("n : DATE;", "n", "~D#0s")]
#[case::time_of_day_midnight("n : TOD;", "n", "~TOD#0ms")]
#[case::date_and_time_epoch("n : DT;", "n", "~DT#0s")]
#[case::empty_string("n : WSTRING;", "n", "~\"\"")]
fn apply_when_program_variable_then_scalar_starts_at_rule_value(
    #[case] declaration: &str,
    #[case] variable: &str,
    #[case] expected: &str,
) {
    let library = resolve(&program(&format!("VAR {declaration} END_VAR")));

    assert_eq!(value(&library, "main", variable), expected);
}

#[spec_test(REQ_IV_analyzer_021)]
fn apply_when_subrange_declares_default_then_variable_starts_at_it() {
    let library = resolve(&program("VAR r : R; END_VAR"));

    assert_eq!(value(&library, "main", "r"), "~50");
}

#[spec_test(REQ_IV_analyzer_022)]
fn apply_when_subrange_in_program_function_and_block_then_lower_bound() {
    let source = format!(
        "{TYPES}
FUNCTION f : DINT
VAR r : R2; END_VAR
f := r;
END_FUNCTION
FUNCTION_BLOCK fb
VAR_OUTPUT o : R2; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR r : R2; b : fb; END_VAR
END_PROGRAM"
    );
    let library = resolve(&source);

    assert_eq!(value(&library, "main", "r"), "~10");
    assert_eq!(value(&library, "f", "r"), "~10");
    assert_eq!(value(&library, "fb", "o"), "~10");
    assert_eq!(value(&library, "main", "b"), "(~o := ~10)");
}

#[spec_test(REQ_IV_analyzer_023)]
fn apply_when_function_var_temp_then_value_and_reset_on_call() {
    let source = "
FUNCTION f : DINT
VAR_INPUT a : DINT; END_VAR
VAR_TEMP t : DINT := 100; END_VAR
t := t + a;
f := t;
END_FUNCTION
PROGRAM main
VAR p : DINT; END_VAR
p := f(a := 1);
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(value(&library, "f", "t"), "100");
    assert!(decl(&library, "f", "t").reset_on_call);
}

/// A function returning `return_type` that assigns no result.
fn function_returning(name: &str, return_type: &str) -> String {
    format!(
        "{TYPES}
FUNCTION {name} : {return_type}
VAR_INPUT a : INT; END_VAR
VAR z : INT; END_VAR
z := a;
END_FUNCTION
PROGRAM main
VAR x : {return_type}; END_VAR
x := {name}(a := 1);
END_PROGRAM"
    )
}

/// The completed initializer of the result of the function `name`.
fn result(library: &Library, name: &str) -> String {
    shown(
        &function(library, name)
            .result
            .variable()
            .unwrap()
            .initializer,
    )
}

#[spec_test(REQ_IV_analyzer_024)]
fn apply_when_function_returns_enumeration_then_result_starts_at_its_default() {
    let library = resolve(&function_returning("fe", "COLOR"));

    assert_eq!(result(&library, "fe"), "~GREEN");
}

#[spec_test(REQ_IV_analyzer_025)]
fn apply_when_function_returns_subrange_then_result_starts_at_lower_bound() {
    let library = resolve(&function_returning("fr", "R2"));

    assert_eq!(result(&library, "fr"), "~10");
}

// --- Arrays ---

#[spec_test(REQ_IV_analyzer_030)]
fn apply_when_array_of_string_with_repetition_then_expanded_with_empty_strings() {
    let library = resolve(&program(
        "VAR s : ARRAY[1..3] OF STRING[5] := [1('ab'), 2()]; END_VAR",
    ));

    assert_eq!(value(&library, "main", "s"), "['ab', ~'', ~'']");
}

#[spec_test(REQ_IV_analyzer_031)]
fn apply_when_fewer_array_values_than_elements_then_filled_with_element_default() {
    let library = resolve(&program(
        "VAR a : ARRAY[1..2, 1..2] OF MYINT := [1, 2(5)]; END_VAR",
    ));

    assert_eq!(value(&library, "main", "a"), "[1, 5, 5, ~7]");
}

#[spec_test(REQ_IV_analyzer_032)]
fn apply_when_array_of_structures_then_every_element_at_structure_default() {
    let library = resolve(&program("VAR a : ARRAY[1..2] OF P; END_VAR"));

    assert_eq!(
        value(&library, "main", "a"),
        "[(~x := ~10, ~y := ~3), (~x := ~10, ~y := ~3)]"
    );
}

#[spec_test(REQ_IV_analyzer_033)]
fn apply_when_array_has_more_values_than_elements_then_p4077_and_initializer_as_written() {
    let library = parse_program(
        &program("VAR a : ARRAY[1..3] OF DINT := [1, 2, 3, 4]; END_VAR"),
        &FileId::default(),
        &CompilerOptions::default(),
    )
    .unwrap();
    let (library, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();

    assert_eq!(codes, vec![Problem::ArrayInitializerTooManyValues.code()]);
    assert_eq!(value(&library, "main", "a"), "[1, 2, 3, 4]");
}

#[spec_test(REQ_IV_analyzer_034)]
fn apply_when_array_of_arrays_then_values_listed_flat() {
    let source = "
TYPE ROW : ARRAY[1..2] OF INT := [8, 9]; END_TYPE
PROGRAM main
VAR a : ARRAY[1..2] OF ROW; b : ARRAY[1..2] OF ROW := [1, 2, 3]; END_VAR
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(value(&library, "main", "a"), "[~8, ~9, ~8, ~9]");
    assert_eq!(value(&library, "main", "b"), "[1, 2, 3, ~9]");
}

// --- Function block instances and references ---

#[spec_test(REQ_IV_analyzer_040)]
fn apply_when_standard_block_instance_initializer_then_member_set_over_defaults() {
    let library = resolve(&program("VAR t : TON := (PT := T#5s); END_VAR"));

    let t = value(&library, "main", "t");
    assert!(t.contains("~IN := ~FALSE"), "{t}");
    assert!(t.contains(" PT := T#5000ms"), "{t}");
}

#[spec_test(REQ_IV_analyzer_041)]
fn apply_when_user_block_instance_initializer_then_applied_over_block_defaults() {
    let source = "
FUNCTION_BLOCK fb
VAR_INPUT limit : INT := 10; gain : INT := 2; END_VAR
VAR count : INT := 1; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR b : fb := (gain := 3); END_VAR
END_PROGRAM";
    let library = resolve(source);

    assert_eq!(
        value(&library, "main", "b"),
        "(~limit := ~10, gain := 3, ~count := ~1)"
    );
}

#[spec_test(REQ_IV_analyzer_042)]
fn apply_when_reference_variables_then_null_or_target() {
    let source = "
PROGRAM main
VAR
  x : INT;
  n : REF_TO INT;
  e : REF_TO INT := NULL;
  r : REF_TO INT := REF(x);
END_VAR
END_PROGRAM";
    let library = resolve_with(source, &edition3_options());

    assert_eq!(value(&library, "main", "n"), "~NULL");
    assert_eq!(value(&library, "main", "e"), "NULL");
    assert_eq!(value(&library, "main", "r"), "REF(x)");
}

// --- Re-initialization ---

#[spec_test(REQ_IV_analyzer_050)]
fn apply_when_pou_variables_then_only_function_and_method_locals_reset_on_call() {
    let source = "
FUNCTION_BLOCK fb
VAR count : INT; END_VAR
METHOD bump : INT
VAR m : INT := 6; END_VAR
bump := m;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION f : DINT
VAR_INPUT a : DINT; END_VAR
VAR v : DINT; END_VAR
VAR_TEMP t : DINT; END_VAR
f := a + v + t;
END_FUNCTION
PROGRAM main
VAR p : DINT; b : fb; END_VAR
p := f(a := 1);
END_PROGRAM";
    let library = resolve_with(source, &fb_inheritance_options());

    assert!(decl(&library, "f", "v").reset_on_call);
    assert!(decl(&library, "f", "t").reset_on_call);
    assert!(decl(&library, "fb.bump", "m").reset_on_call);
    assert!(
        function(&library, "f")
            .result
            .variable()
            .unwrap()
            .reset_on_call
    );
    assert!(!decl(&library, "f", "a").reset_on_call);
    assert!(!decl(&library, "fb", "count").reset_on_call);
    assert!(!decl(&library, "main", "p").reset_on_call);
}

#[spec_test(REQ_IV_analyzer_006)]
fn apply_when_top_level_global_of_structure_type_then_completed_as_structure() {
    let source = "
TYPE PAIR : STRUCT a : DINT := 4; b : DINT; END_STRUCT; END_TYPE
VAR_GLOBAL phys : PAIR; END_VAR
PROGRAM main
VAR result : DINT; END_VAR
result := phys.a;
END_PROGRAM
";
    let options = CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Rusty);
    let library = resolve_with(source, &options);

    assert!(matches!(
        decl(&library, "", "phys").initializer,
        InitialValueAssignmentKind::Structure(_)
    ));
    assert_eq!(value(&library, "", "phys"), "(~a := ~4, ~b := ~0)");
}
