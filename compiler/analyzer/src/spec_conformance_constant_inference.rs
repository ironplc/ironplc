//! Spec conformance tests for constant variable inference (analyzer-owned
//! requirements): which never-written declarations become `CONSTANT`, and
//! what counts as a write.
//!
//! Each test is annotated with `#[spec_test(REQ_CVI_analyzer_NNN)]`, which
//! adds `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every analyzer-owned requirement has a test.
//!
//! See `specs/design/constant-variable-inference.md`.

use ironplc_dsl::common::{DeclarationQualifier, Library};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::stages::analyze;
use crate::test_helpers::declaration_qualifiers;

/// Analyze `program` with the default options and return the library and the
/// problem codes analysis produced.
fn analyze_with(program: &str, options: &CompilerOptions) -> (Library, Vec<String>) {
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    let codes = context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect();
    (library, codes)
}

fn analyze_default(program: &str) -> (Library, Vec<String>) {
    analyze_with(program, &CompilerOptions::default())
}

/// The qualifier of the one declaration named `name`.
fn qualifier(library: &Library, name: &str) -> DeclarationQualifier {
    let mut found = declaration_qualifiers(library, name);
    assert_eq!(1, found.len(), "expected one declaration of {name}");
    found.remove(0)
}

/// A program whose body is `body`, declaring `x : INT := 5` and a few helper
/// variables for the body to use.
fn program_with_body(body: &str) -> String {
    format!(
        "
FUNCTION Twice : INT
VAR_INPUT
    n : INT;
END_VAR
    Twice := n * 2;
END_FUNCTION

FUNCTION Bump : INT
VAR_IN_OUT
    io : INT;
END_VAR
    io := io + 1;
    Bump := io;
END_FUNCTION

TYPE
    Point : STRUCT a : INT; b : INT; END_STRUCT;
END_TYPE

PROGRAM main
VAR
    x : INT := 5;
    y : INT;
    arr : ARRAY[0..3] OF INT;
    pt : Point;
    bits : WORD := 16#00FF;
    i : INT;
    timer : TON;
END_VAR
{body}
END_PROGRAM"
    )
}

// ---------------------------------------------------------------------------
// Which declarations are marked
// ---------------------------------------------------------------------------

/// REQ-CVI-analyzer-001: A `VAR` or `VAR_TEMP` declaration with no qualifier,
/// a symbolic identifier, an initializer and no write is marked `CONSTANT`.
#[spec_test(REQ_CVI_analyzer_001)]
fn analyzer_spec_req_cvi_001_unwritten_var_and_var_temp_are_marked_constant() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_Greeter
VAR
    greeting : STRING := 'Hello';
    count : INT := 0;
END_VAR
VAR_TEMP
    scratch : INT := 1;
END_VAR
    count := LEN(greeting) + scratch;
END_FUNCTION_BLOCK",
    );
    assert_eq!(
        DeclarationQualifier::Constant,
        qualifier(&library, "greeting")
    );
    assert_eq!(
        DeclarationQualifier::Constant,
        qualifier(&library, "scratch")
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "count")
    );
}

/// REQ-CVI-analyzer-002: A declaration without an initializer is never marked.
#[spec_test(REQ_CVI_analyzer_002)]
fn analyzer_spec_req_cvi_002_declaration_without_initializer_is_not_marked() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR
    x : INT;
    y : INT;
END_VAR
    y := x;
END_PROGRAM",
    );
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-003: A located declaration is never marked.
#[spec_test(REQ_CVI_analyzer_003)]
fn analyzer_spec_req_cvi_003_located_declaration_is_not_marked() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR
    sensor AT %IX0.0 : BOOL := TRUE;
END_VAR
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "sensor")
    );
}

/// REQ-CVI-analyzer-004: `VAR_INPUT`, `VAR_OUTPUT` and `VAR_IN_OUT`
/// declarations are never marked, even with an initializer and no write.
#[spec_test(REQ_CVI_analyzer_004)]
fn analyzer_spec_req_cvi_004_parameter_declarations_are_not_marked() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_Params
VAR_INPUT
    in : INT := 1;
END_VAR
VAR_OUTPUT
    out : INT := 2;
END_VAR
VAR_IN_OUT
    io : INT;
END_VAR
END_FUNCTION_BLOCK",
    );
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "in"));
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "out")
    );
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "io"));
}

/// REQ-CVI-analyzer-005: A declaration that already carries a qualifier is
/// left unchanged.
#[rstest]
#[case::constant("CONSTANT", DeclarationQualifier::Constant)]
#[case::retain("RETAIN", DeclarationQualifier::Retain)]
#[case::non_retain("NON_RETAIN", DeclarationQualifier::NonRetain)]
#[spec_test(REQ_CVI_analyzer_005)]
fn analyzer_spec_req_cvi_005_qualified_declaration_is_unchanged(
    #[case] keyword: &str,
    #[case] expected: DeclarationQualifier,
) {
    let program = format!(
        "
PROGRAM main
VAR {keyword}
    x : INT := 5;
END_VAR
END_PROGRAM"
    );
    let (library, _) = analyze_default(&program);
    assert_eq!(expected, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-006: A function-block instance is never marked, whether
/// or not it is ever called.
#[spec_test(REQ_CVI_analyzer_006)]
fn analyzer_spec_req_cvi_006_function_block_instance_is_not_marked() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR
    idle : TON;
    busy : TON := (PT := T#1s);
END_VAR
    busy(IN := TRUE);
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "idle")
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "busy")
    );
}

// ---------------------------------------------------------------------------
// What counts as a write
// ---------------------------------------------------------------------------

/// REQ-CVI-analyzer-010: An assignment to the variable or to any element of
/// it counts as a write of the whole variable.
#[rstest]
#[case::whole("x := 1;", "x")]
#[case::array_element("arr[1] := 1;", "arr")]
#[case::structure_field("pt.a := 1;", "pt")]
#[case::bit_access("bits.3 := TRUE;", "bits")]
#[case::partial_access("bits.%B0 := 16#12;", "bits")]
#[spec_test(REQ_CVI_analyzer_010)]
fn analyzer_spec_req_cvi_010_assignment_to_any_part_is_a_write(
    #[case] body: &str,
    #[case] name: &str,
) {
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    let (library, _) = analyze_with(&program_with_body(body), &options);
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, name));
}

/// REQ-CVI-analyzer-011: The control variable of a `FOR` loop is written.
#[spec_test(REQ_CVI_analyzer_011)]
fn analyzer_spec_req_cvi_011_for_control_variable_is_a_write() {
    let (library, _) = analyze_default(&program_with_body(
        "FOR x := 0 TO 3 DO y := y + x; END_FOR;",
    ));
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-012: An output binding `Q => v` writes `v`.
#[spec_test(REQ_CVI_analyzer_012)]
fn analyzer_spec_req_cvi_012_output_binding_is_a_write() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR
    timer : TON;
    done : BOOL := FALSE;
END_VAR
    timer(IN := TRUE, PT := T#1s, Q => done);
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "done")
    );
}

/// REQ-CVI-analyzer-013: An argument bound to a `VAR_IN_OUT` parameter is
/// written; one bound to a `VAR_INPUT` parameter is not.
#[rstest]
#[case::function_input("y := Twice(x);", DeclarationQualifier::Constant)]
#[case::function_input_named("y := Twice(n := x);", DeclarationQualifier::Constant)]
#[case::stdlib_input("y := ABS(x);", DeclarationQualifier::Constant)]
#[case::fb_input(
    "timer(IN := TRUE, PT := T#1s); y := x;",
    DeclarationQualifier::Constant
)]
#[case::function_in_out("y := Bump(x);", DeclarationQualifier::Unspecified)]
#[case::function_in_out_named("y := Bump(io := x);", DeclarationQualifier::Unspecified)]
#[spec_test(REQ_CVI_analyzer_013)]
fn analyzer_spec_req_cvi_013_in_out_argument_is_a_write_and_input_is_not(
    #[case] body: &str,
    #[case] expected: DeclarationQualifier,
) {
    let (library, _) = analyze_default(&program_with_body(body));
    assert_eq!(expected, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-014: Taking a variable's address with `REF()`, `ADR()`
/// or `REF=` counts as a write.
#[rstest]
#[case::ref_function("p := REF(x);")]
#[case::adr_function("p := ADR(x);")]
#[case::ref_bind("r REF= x;")]
#[spec_test(REQ_CVI_analyzer_014)]
fn analyzer_spec_req_cvi_014_address_of_is_a_write(#[case] body: &str) {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_pointer_to: true,
        allow_adr: true,
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let program = format!(
        "
PROGRAM main
VAR
    x : INT := 5;
    p : REF_TO INT;
    r : REFERENCE TO INT;
END_VAR
    {body}
END_PROGRAM"
    );
    let (library, _) = analyze_with(&program, &options);
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-015: An argument to a callee whose parameter directions
/// cannot be determined is taken to be written.
#[spec_test(REQ_CVI_analyzer_015)]
fn analyzer_spec_req_cvi_015_argument_to_unknown_callee_is_a_write() {
    let (library, _) = analyze_default(&program_with_body("y := Undeclared(x);"));
    assert_eq!(DeclarationQualifier::Unspecified, qualifier(&library, "x"));
}

/// REQ-CVI-analyzer-016: A function-block instance initializer that sets a
/// member writes that member.
#[spec_test(REQ_CVI_analyzer_016)]
fn analyzer_spec_req_cvi_016_instance_initializer_writes_the_member() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_Counter
VAR
    limit : INT := 10;
    step : INT := 1;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    counter : FB_Counter := (limit := 20);
END_VAR
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "limit")
    );
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "step"));
}

/// REQ-CVI-analyzer-017: A program connection sink in a configuration writes
/// the global it targets.
#[spec_test(REQ_CVI_analyzer_017)]
fn analyzer_spec_req_cvi_017_program_connection_sink_writes_the_global() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR_OUTPUT
    result : INT;
END_VAR
END_PROGRAM
CONFIGURATION config
    VAR_GLOBAL
        latest : INT := 0;
        untouched : INT := 0;
    END_VAR
    RESOURCE res ON PLC
        TASK fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM prog WITH fast : main(result => latest);
    END_RESOURCE
END_CONFIGURATION",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "latest")
    );
    assert_eq!(
        DeclarationQualifier::Constant,
        qualifier(&library, "untouched")
    );
}

/// REQ-CVI-analyzer-018: A `VAR_ACCESS` path that is not `READ_ONLY` writes
/// the variable at its end.
#[rstest]
#[case::read_write("READ_WRITE")]
#[case::unspecified("")]
#[spec_test(REQ_CVI_analyzer_018)]
fn analyzer_spec_req_cvi_018_writable_access_path_is_a_write(#[case] direction: &str) {
    let program = format!(
        "
PROGRAM main
VAR
    limit : INT := 10;
END_VAR
VAR_ACCESS
    LIMIT_ACCESS : limit : INT {direction};
END_VAR
END_PROGRAM"
    );
    let (library, _) = analyze_default(&program);
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "limit")
    );
}

/// REQ-CVI-analyzer-019: An SFC action association writes the action name
/// and each indicator variable.
#[spec_test(REQ_CVI_analyzer_019)]
fn analyzer_spec_req_cvi_019_action_association_writes_name_and_indicators() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_Seq
VAR
    flag : BOOL := FALSE;
    shown : BOOL := FALSE;
    count : INT := 0;
END_VAR
    INITIAL_STEP Start:
    END_STEP
    TRANSITION FROM Start TO Run
        := TRUE;
    END_TRANSITION
    STEP Run:
        flag(N, shown);
    END_STEP
    ACTION flag:
        count := 0;
    END_ACTION
END_FUNCTION_BLOCK",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "flag")
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "shown")
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "count")
    );
}

/// REQ-CVI-analyzer-020: Assigning a member of a function-block instance
/// from outside writes the member as well as the instance.
#[spec_test(REQ_CVI_analyzer_020)]
fn analyzer_spec_req_cvi_020_member_assignment_writes_the_member() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_Counter
VAR
    limit : INT := 10;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    counter : FB_Counter;
END_VAR
    counter.limit := 20;
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Unspecified,
        qualifier(&library, "limit")
    );
}

/// REQ-CVI-analyzer-021: A write to a name anywhere in the library blocks
/// every declaration of that name, in every unit.
#[spec_test(REQ_CVI_analyzer_021)]
fn analyzer_spec_req_cvi_021_write_in_one_unit_blocks_same_name_everywhere() {
    let (library, _) = analyze_default(
        "
FUNCTION_BLOCK FB_A
VAR
    shared : INT := 1;
    own : INT := 1;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_B
VAR
    shared : INT := 2;
END_VAR
    shared := 3;
END_FUNCTION_BLOCK",
    );
    assert_eq!(
        vec![
            DeclarationQualifier::Unspecified,
            DeclarationQualifier::Unspecified
        ],
        declaration_qualifiers(&library, "shared")
    );
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "own"));
}

// ---------------------------------------------------------------------------
// Globals and externals
// ---------------------------------------------------------------------------

const GLOBAL_PROGRAM: &str = "
PROGRAM main
VAR_EXTERNAL
    limit : INT;
END_VAR
VAR
    local : INT := 1;
END_VAR
    local := limit;
END_PROGRAM
FUNCTION_BLOCK FB_User
VAR_EXTERNAL
    limit : INT;
END_VAR
VAR
    copy : INT := 0;
END_VAR
    copy := limit;
END_FUNCTION_BLOCK
CONFIGURATION config
    VAR_GLOBAL
        limit : INT := 10;
    END_VAR
    RESOURCE res ON PLC
        TASK fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM prog WITH fast : main;
    END_RESOURCE
END_CONFIGURATION";

/// REQ-CVI-analyzer-030: An unwritten global with an initializer is marked
/// together with every `VAR_EXTERNAL` declaration of it, and no diagnostic
/// results.
#[spec_test(REQ_CVI_analyzer_030)]
fn analyzer_spec_req_cvi_030_unwritten_global_and_its_externals_are_marked() {
    let (library, codes) = analyze_default(GLOBAL_PROGRAM);
    assert_eq!(
        vec![
            DeclarationQualifier::Constant,
            DeclarationQualifier::Constant,
            DeclarationQualifier::Constant,
        ],
        declaration_qualifiers(&library, "limit")
    );
    assert!(codes.is_empty(), "unexpected diagnostics {codes:?}");
}

/// REQ-CVI-analyzer-031: When the global cannot be marked, neither its
/// externals nor a same-named local declaration are.
#[spec_test(REQ_CVI_analyzer_031)]
fn analyzer_spec_req_cvi_031_unmarkable_global_leaves_externals_and_locals_alone() {
    let program = GLOBAL_PROGRAM
        .replacen(
            "    copy := limit;",
            "    copy := limit;\n    limit := 0;",
            1,
        )
        .replacen(
            "VAR\n    local : INT := 1;",
            "VAR\n    local : INT := 1;\n    limit_shadow : INT := 1;",
            1,
        )
        .replace("limit_shadow", "limit");
    let (library, codes) = analyze_default(&program);
    assert!(
        declaration_qualifiers(&library, "limit")
            .iter()
            .all(|q| *q == DeclarationQualifier::Unspecified),
        "no declaration of a written global may be marked"
    );
    assert!(codes.is_empty(), "unexpected diagnostics {codes:?}");
}

// ---------------------------------------------------------------------------
// Pipeline position
// ---------------------------------------------------------------------------

/// REQ-CVI-analyzer-040: The library `analyze` returns carries the inferred
/// qualifiers.
#[spec_test(REQ_CVI_analyzer_040)]
fn analyzer_spec_req_cvi_040_analyzed_library_carries_inferred_qualifiers() {
    let (library, _) = analyze_default(
        "
PROGRAM main
VAR
    greeting : STRING := 'Hello';
    n : INT;
END_VAR
    n := LEN(greeting);
END_PROGRAM",
    );
    assert_eq!(
        DeclarationQualifier::Constant,
        qualifier(&library, "greeting")
    );
}

/// REQ-CVI-analyzer-041: Marking introduces no diagnostics -- in particular
/// neither P4008 (constant must have initializer) nor P4009 (external must be
/// constant) -- for a program that analyzes cleanly.
#[spec_test(REQ_CVI_analyzer_041)]
fn analyzer_spec_req_cvi_041_marking_introduces_no_diagnostics() {
    let (library, codes) = analyze_default(
        "
TYPE Color : (Red, Green); END_TYPE
FUNCTION_BLOCK FB_Mixed
VAR
    greeting : STRING := 'Hello';
    color : Color := Green;
    table : ARRAY[0..1] OF INT := [1, 2];
    timer : TON;
    n : INT;
END_VAR
    timer(IN := TRUE, PT := T#1s);
    n := LEN(greeting);
END_FUNCTION_BLOCK",
    );
    assert_eq!(
        DeclarationQualifier::Constant,
        qualifier(&library, "greeting")
    );
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "color"));
    assert_eq!(DeclarationQualifier::Constant, qualifier(&library, "table"));
    assert!(codes.is_empty(), "unexpected diagnostics {codes:?}");
}
