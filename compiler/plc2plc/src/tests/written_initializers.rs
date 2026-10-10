//! Rendering of initializers that hold parts the program did not write.
//!
//! A part of an initializer with a synthesized span is one the compiler
//! supplied rather than parsed. A rendering shows the program as written,
//! so those parts are left out. Each test here analyzes a source, marks
//! chosen parts of its initializers synthesized, and checks the rendering
//! of what is left.

use super::common::*;

use dsl::common::{
    ArrayInitialElementKind, CharacterStringLiteral, ConstantKind, InitialValueAssignmentKind,
    IntegerLiteral, Library, LibraryElementKind, SignedInteger, StructInitialValueAssignmentKind,
    StructureElementInit,
};
use dsl::core::{Id, SourceSpan};
use dsl::fold::Fold;

/// Marks synthesized every span that lies within one of `regions`, each a
/// substring that occurs once in the source the library was parsed from.
struct Synthesize {
    regions: Vec<(usize, usize)>,
}

impl Fold<()> for Synthesize {
    fn fold_source_span(&mut self, span: SourceSpan) -> Result<SourceSpan, ()> {
        let within = self
            .regions
            .iter()
            .any(|(start, end)| *start <= span.start && span.end <= *end);
        Ok(if within {
            SourceSpan::synthesized()
        } else {
            span
        })
    }

    fn fold_character_string_literal(
        &mut self,
        mut node: CharacterStringLiteral,
    ) -> Result<CharacterStringLiteral, ()> {
        node.span = self.fold_source_span(node.span)?;
        Ok(node)
    }
}

/// Analyzes `source` and marks synthesized the parts of it within each of
/// `regions`.
fn analyze_and_synthesize(source: &str, regions: &[&str], options: &CompilerOptions) -> Library {
    let regions = regions
        .iter()
        .map(|region| {
            let start = source
                .find(region)
                .unwrap_or_else(|| panic!("{region:?} is not in the source"));
            assert_eq!(
                source.matches(region).count(),
                1,
                "{region:?} occurs more than once in the source"
            );
            (start, start + region.len())
        })
        .collect();
    let (library, _) = analyze_and_render_with_types(source, options);
    Synthesize { regions }.fold_library(library).unwrap()
}

/// Requires the rendering of `library` to be the rendering of `expected`
/// after analysis -- the program as written, without the synthesized parts
/// -- and to re-parse. Returns the rendered text.
fn assert_renders_as(library: &Library, expected: &str, options: &CompilerOptions) -> String {
    let rendered = write_to_string(library).unwrap();
    let (expected, _) = analyze_and_render_with_types(expected, options);
    assert_eq!(rendered, write_to_string(&expected).unwrap());
    parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    rendered
}

#[test]
fn write_to_string_when_scalar_initial_values_synthesized_then_leaves_out_assignment() {
    let source = "
TYPE
    Color : (Red, Green);
    Level : STRUCT r : INT (0..10) := 8; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    a : INT := 7;
    s : STRING[5] := 'abc';
    c : Color := Color#Green;
    e : (Up, Down) := Down;
    p : REF_TO INT := NULL;
END_VAR
    a := a;
    s := s;
    c := c;
    e := e;
END_PROGRAM";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };

    let library = analyze_and_synthesize(
        source,
        &["7", "8", "'abc'", "Color#Green", ":= Down", "NULL"],
        &options,
    );

    assert_renders_as(
        &library,
        "
TYPE
    Color : (Red, Green);
    Level : STRUCT r : INT (0..10); END_STRUCT;
END_TYPE
PROGRAM main
VAR
    a : INT;
    s : STRING[5];
    c : Color;
    e : (Up, Down);
    p : REF_TO INT;
END_VAR
    a := a;
    s := s;
    c := c;
    e := e;
END_PROGRAM",
        &options,
    );
}

#[test]
fn write_to_string_when_some_members_synthesized_then_writes_written_members() {
    let source = "
TYPE
    Inner : STRUCT x : INT; y : INT; END_STRUCT;
    Outer : STRUCT inner : Inner; values : ARRAY[1..3] OF INT; z : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    p : Inner := (x := 1, y := 2);
    q : Inner := (x := 3, y := 4);
    o : Outer := (inner := (x := 5, y := 6), values := [7, 8, 9], z := 10);
END_VAR
END_PROGRAM";
    let options = CompilerOptions::default();

    let library = analyze_and_synthesize(
        source,
        &["y := 2", "x := 3, y := 4", "y := 6", "8, 9", "z := 10"],
        &options,
    );

    assert_renders_as(
        &library,
        "
TYPE
    Inner : STRUCT x : INT; y : INT; END_STRUCT;
    Outer : STRUCT inner : Inner; values : ARRAY[1..3] OF INT; z : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    p : Inner := (x := 1);
    q : Inner;
    o : Outer := (inner := (x := 5), values := [7]);
END_VAR
END_PROGRAM",
        &options,
    );
}

#[test]
fn write_to_string_when_array_elements_synthesized_then_writes_up_to_last_written() {
    let source = "
PROGRAM main
VAR
    a : ARRAY[1..4] OF INT := [10, 20, 30, 40];
    b : ARRAY[1..2] OF INT := [50, 60];
END_VAR
    a[1] := 0;
    b[1] := 0;
END_PROGRAM";
    let options = CompilerOptions::default();

    let library = analyze_and_synthesize(source, &["20", "40", "50, 60"], &options);

    // An element supplied before the last written one keeps its place.
    assert_renders_as(
        &library,
        "
PROGRAM main
VAR
    a : ARRAY[1..4] OF INT := [10, 20, 30];
    b : ARRAY[1..2] OF INT;
END_VAR
    a[1] := 0;
    b[1] := 0;
END_PROGRAM",
        &options,
    );
}

#[test]
fn write_to_string_when_function_block_members_synthesized_then_writes_written_members() {
    let source = "
FUNCTION_BLOCK Counter
VAR_INPUT
    preset : INT;
    delta : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    first : Counter := (preset := 5, delta := 6);
    second : Counter := (preset := 7);
END_VAR
END_PROGRAM";
    let options = CompilerOptions::default();

    let library = analyze_and_synthesize(source, &["delta := 6", "preset := 7"], &options);

    let rendered = assert_renders_as(
        &library,
        "
FUNCTION_BLOCK Counter
VAR_INPUT
    preset : INT;
    delta : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    first : Counter := (preset := 5);
    second : Counter;
END_VAR
END_PROGRAM",
        &options,
    );
    assert!(
        rendered.contains("first : Counter := ( preset := 5 );"),
        "Rendered:\n{rendered}"
    );
}

/// A structure member `name := value` with `span` on both its parts.
fn member(name: &str, value: &str, span: SourceSpan) -> StructureElementInit {
    StructureElementInit {
        name: Id::from(name).with_position(span.clone()),
        init: StructInitialValueAssignmentKind::Constant(ConstantKind::IntegerLiteral(
            IntegerLiteral {
                value: SignedInteger::new(value, span).unwrap(),
                data_type: None,
            },
        )),
    }
}

#[test]
fn write_to_string_when_structure_elements_synthesized_then_writes_written_members() {
    // The parser has no syntax for an array of structures' initializer; the
    // compiler makes one when it completes the initializer, so this builds
    // it by hand.
    let source = "
TYPE
    Point : STRUCT x : INT; y : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    c : ARRAY[1..3] OF Point;
END_VAR
END_PROGRAM";
    let (mut library, _) = analyze_and_render_with_types(source, &CompilerOptions::default());
    let LibraryElementKind::ProgramDeclaration(program) = &mut library.elements[1] else {
        panic!("Expected the program");
    };
    let InitialValueAssignmentKind::Array(array) = &mut program.variables[0].initializer else {
        panic!("Expected an array initializer");
    };
    let written = SourceSpan::range(1, 2);
    let synthesized = SourceSpan::synthesized();
    array.initial_values = vec![
        ArrayInitialElementKind::Structure(vec![
            member("x", "1", written.clone()),
            member("y", "2", synthesized.clone()),
        ]),
        ArrayInitialElementKind::Structure(vec![member("x", "3", written)]),
        ArrayInitialElementKind::Structure(vec![member("y", "4", synthesized)]),
    ];

    let rendered = write_to_string(&library).unwrap();

    assert!(
        rendered.contains("c : ARRAY [ 1.. 3 ] OF Point := [ ( x := 1 ) , ( x := 3 ) ];"),
        "Rendered:\n{rendered}"
    );
}

/// Parses `source`, analyzes it, renders the analyzed library, and requires
/// the rendering to re-parse to the library `source` parses to.
fn assert_analyzed_round_trips(source: &str, options: &CompilerOptions) -> String {
    let parsed = parse_program(source, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Source did not parse: {e:?}\n{source}"));
    let (analyzed, _) = analyze_and_render_with_types(source, options);
    let rendered = write_to_string(&analyzed).unwrap();

    let reparsed = parse_program(&rendered, &FileId::default(), options)
        .unwrap_or_else(|e| panic!("Rendered output did not re-parse: {e:?}\n{rendered}"));
    assert_eq!(
        parsed, reparsed,
        "Round trip changed the AST. Rendered:\n{rendered}"
    );

    rendered
}

#[test]
fn write_to_string_when_structure_member_is_array_then_round_trips() {
    let source = "
TYPE
    Inner : STRUCT x : INT; END_STRUCT;
    Outer : STRUCT values : ARRAY[1..2] OF INT; inner : Inner; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    o : Outer := (values := [1, 2], inner := (x := 3));
END_VAR
END_PROGRAM";

    let rendered = assert_analyzed_round_trips(source, &CompilerOptions::default());

    assert!(
        rendered.contains("( values := [ 1 , 2 ] , inner := ( x := 3 ) )"),
        "Rendered:\n{rendered}"
    );
}

#[test]
fn write_to_string_when_function_block_instance_has_member_initializer_then_round_trips() {
    let source = "
FUNCTION_BLOCK Counter
VAR_INPUT
    preset : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    counter : Counter := (preset := 5);
END_VAR
END_PROGRAM";

    let rendered = assert_analyzed_round_trips(source, &CompilerOptions::default());

    assert!(
        rendered.contains("counter : Counter := ( preset := 5 );"),
        "Rendered:\n{rendered}"
    );
}
