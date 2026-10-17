//! End-to-end tests for the value a variable of every elementary type other
//! than a string starts with, in every kind of declaration that can give it
//! one: a variable, a global, a function's local, a structure's initializer,
//! a field of a structure type, an array's initializer, written out and
//! repeated, a variable of a function block, and the initializer of a
//! function block instance.
//!
//! Each test compiles one program that gives the same literal in each of
//! those declarations, runs a scan and reads every one back. The literal is
//! typed by its declaration, so a declaration that typed it wider or narrower
//! than the others would start at another value.
//!
//! The `rstest` anchors run every type at the ends of its range and without
//! an initial value. The property test adds arbitrary values.
//!
//! The values a declared type gives its variables, and the other shapes an
//! initializer takes, are in `end_to_end_initial_values.rs`.

use ironplc_parser::options::CompilerOptions;
use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;

use crate::common::scalar::{
    self, scalars, Mismatch, Num, Scalar, BOOL, BYTE, DATE, DINT, DT, DWORD, INT, LDATE, LDT, LINT,
    LREAL, LTIME, LTOD, LWORD, REAL, SINT, TIME, TOD, UDINT, UINT, ULINT, USINT, WORD,
};
use crate::common::{drive_fb, expect, run, FbStep, Snapshot};

/// The variables that each read one declaration back.
const DECLARATIONS: [&str; 9] = [
    "in_variable",
    "in_global",
    "in_local",
    "in_field",
    "in_type_field",
    "in_element",
    "in_repeated_element",
    "in_block_variable",
    "in_block_input",
];

/// The program that declares `initializer` for a value of type `ty` in every
/// kind of declaration and reads each back into one of [`DECLARATIONS`].
/// Without a `literal`, no declaration gives an initial value.
fn program(ty: Scalar, literal: Option<&str>) -> String {
    let initializer = |value: fn(&str) -> String| {
        literal.map_or(String::new(), |literal| format!(" := {}", value(literal)))
    };
    format!(
        "
TYPE Holder : STRUCT field : {ty}; END_STRUCT; END_TYPE
TYPE Preset : STRUCT field : {ty}{scalar}; END_STRUCT; END_TYPE

FUNCTION from_local : {ty}
  VAR local : {ty}{scalar}; END_VAR
  from_local := local;
END_FUNCTION

FUNCTION_BLOCK Block
  VAR_INPUT given : {ty}; END_VAR
  VAR_OUTPUT seen : {ty}; own_seen : {ty}; END_VAR
  VAR own : {ty}{scalar}; END_VAR
  seen := given;
  own_seen := own;
END_FUNCTION_BLOCK

CONFIGURATION config
  VAR_GLOBAL shared : {ty}{scalar}; END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL shared : {ty}; END_VAR
  VAR
    in_variable : {ty}{scalar};
    holder : Holder{structure};
    preset : Preset;
    elements : ARRAY[0..2] OF {ty}{elements};
    repeated : ARRAY[0..2] OF {ty}{repeated};
    block : Block{block};
    in_global : {ty}; in_local : {ty}; in_field : {ty}; in_type_field : {ty};
    in_element : {ty}; in_repeated_element : {ty};
    in_block_variable : {ty}; in_block_input : {ty};
  END_VAR
  in_global := shared;
  in_local := from_local();
  in_field := holder.field;
  in_type_field := preset.field;
  in_element := elements[2];
  in_repeated_element := repeated[1];
  block();
  in_block_variable := block.own_seen;
  in_block_input := block.seen;
END_PROGRAM
",
        ty = ty.name,
        scalar = initializer(|literal| literal.to_string()),
        structure = initializer(|literal| format!("(field := {literal})")),
        elements = initializer(|literal| format!("[{literal}, {literal}, {literal}]")),
        repeated = initializer(|literal| format!("[3({literal})]")),
        block = initializer(|literal| format!("(given := {literal})")),
    )
}

/// Declares each of `values` as the initial value of every declaration, and
/// returns the declarations that do not start with it. `None` declares no
/// initial value, which starts every declaration at zero.
fn mismatches(ty: Scalar, values: &[Option<Num>]) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();
    for value in values {
        let literal = value.and_then(|value| ty.literal(value));
        let expected = ty.value(value.unwrap_or(ty.zero()));
        let snapshot = Snapshot::run(&program(ty, literal.as_deref()), &scalar::options());
        for declaration in DECLARATIONS {
            mismatches.extend(Mismatch::of(
                || format!("{declaration} of {} declared {literal:?}", ty.name),
                snapshot.read(declaration),
                expected.clone(),
            ));
        }
    }
    mismatches
}

/// The edge values of `ty` that a literal can write, and no initial value.
fn edges_and_none(ty: Scalar) -> Vec<Option<Num>> {
    ty.edges()
        .into_iter()
        .filter(|&edge| ty.literal(edge).is_some())
        .map(Some)
        .chain([None])
        .collect()
}

/// Any type, with any value of it that a literal can write.
fn any_type_and_value() -> impl Strategy<Value = (Scalar, Num)> {
    select(scalars()).prop_flat_map(|ty| {
        let written = ty.any().prop_filter("a literal writes it", move |&value| {
            ty.literal(value).is_some()
        });
        (Just(ty), written)
    })
}

#[rstest]
fn end_to_end_when_declared_at_ends_of_range_then_every_declaration_starts_with_it(
    #[values(
        BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL, LREAL, BYTE, WORD, DWORD,
        LWORD, TIME, DATE, TOD, DT, LTIME, LDATE, LTOD, LDT
    )]
    ty: Scalar,
) {
    assert_eq!(mismatches(ty, &edges_and_none(ty)), []);
}

proptest! {
    // Each case compiles a program, and the anchors above already run every
    // type at the ends of its range, so a quarter of the default cases is
    // enough.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn end_to_end_when_declared_with_arbitrary_value_then_every_declaration_starts_with_it(
        (ty, value) in any_type_and_value(),
    ) {
        prop_assert_eq!(mismatches(ty, &[Some(value)]), []);
    }
}

/// An integer literal in the initializer of a real array, structure field or
/// function block input is the real of that value.
const INTEGER_LITERAL_FOR_REAL: &str = "
TYPE Holder : STRUCT field : LREAL; END_STRUCT; END_TYPE

FUNCTION_BLOCK Block
  VAR_INPUT given : REAL; END_VAR
  VAR_OUTPUT seen : REAL; END_VAR
  seen := given;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    cells : ARRAY[1..3] OF REAL := [1, 2.5, 3];
    holder : Holder := (field := 3);
    block : Block := (given := 2);
    in_element : REAL; in_field : LREAL; in_block_input : REAL;
  END_VAR
  in_element := cells[1] + cells[2];
  in_field := holder.field;
  block();
  in_block_input := block.seen;
END_PROGRAM
";

/// A constant holds its initial value, of any type, and may be a subscript.
const CONSTANTS: &str = "
PROGRAM main
  VAR CONSTANT WHOLE : DINT := 4; FRACTION : REAL := 2.5; TEXT : STRING[3] := 'abc'; END_VAR
  VAR
    cells : ARRAY[1..5] OF INT;
    doubled : DINT; copied : REAL; length : DINT; element : INT;
  END_VAR
  doubled := WHOLE * 2;
  copied := FRACTION;
  length := LEN(TEXT);
  cells[WHOLE] := 3;
  element := cells[4];
END_PROGRAM
";

#[rstest]
#[case::integer_literal_for_real(
    INTEGER_LITERAL_FOR_REAL,
    &[run(0), expect("in_element", 3.5f32), expect("in_field", 3.0f64), expect("in_block_input", 2.0f32)]
)]
#[case::constants(
    CONSTANTS,
    &[run(0), expect("doubled", 8), expect("copied", 2.5f32), expect("length", 3), expect("element", 3)]
)]
fn end_to_end_when_initializer_of_real_or_constant_then_stored_at_declared_type(
    #[case] source: &str,
    #[case] steps: &[FbStep],
) {
    drive_fb(source, &CompilerOptions::default(), steps);
}
