//! Unit tests for `stages`.

use crate::stages::analyze;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_test::read_shared_resource;

#[test]
fn analyze_when_first_steps_then_result_is_ok() {
    let lib = parse_shared_library("first_steps.st");
    let res = analyze(&[&lib], &CompilerOptions::default());
    assert!(res.is_ok());
}

#[test]
fn analyze_when_first_steps_semantic_error_then_ok_with_diagnostics() {
    let lib = parse_shared_library("first_steps_semantic_error.st");
    let res = analyze(&[&lib], &CompilerOptions::default());
    let (_library, context) = res.unwrap();
    assert!(context.has_diagnostics());
}

/// Issue #1566: a second file must not displace the first file's
/// diagnostics. Every undefined variable across both files is reported.
#[test]
fn analyze_when_semantic_errors_in_two_files_then_reports_all() {
    let file_a = "
PROGRAM a
VAR
  x : INT;
END_VAR
  x := AAA_ONE;
  x := AAA_TWO;
END_PROGRAM";

    let file_b = "
PROGRAM b
VAR
  y : INT;
END_VAR
  y := BBB_ONE;
END_PROGRAM";

    let options = CompilerOptions::default();
    let library_a = parse_program(file_a, &FileId::from_string("a.st"), &options).unwrap();
    let library_b = parse_program(file_b, &FileId::from_string("b.st"), &options).unwrap();

    let (_library, context) = analyze(&[&library_a, &library_b], &options).unwrap();

    let reported: Vec<&String> = context
        .diagnostics()
        .iter()
        .flat_map(|d| &d.described)
        .collect();
    for expected in ["variable=AAA_ONE", "variable=AAA_TWO", "variable=BBB_ONE"] {
        assert!(
            reported.iter().any(|d| d.as_str() == expected),
            "expected {expected}, got {reported:?}"
        );
    }
}

#[test]
fn analyze_2() {
    let lib = parse_shared_library("main.st");
    let res = analyze(&[&lib], &CompilerOptions::default());
    assert!(res.is_ok());
}

#[test]
fn analyze_when_split_across_multiple_files_then_ok() {
    let program1 = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE";

    let program2 = "
FUNCTION_BLOCK LOGGER
VAR_EXTERNAL CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

    let program1 =
        parse_program(program1, &FileId::default(), &CompilerOptions::default()).unwrap();
    let program2 =
        parse_program(program2, &FileId::default(), &CompilerOptions::default()).unwrap();

    let result = analyze(&[&program1, &program2], &CompilerOptions::default());
    assert!(result.is_ok())
}

fn parse_shared_library(name: &'static str) -> Library {
    let src = read_shared_resource(name);
    parse_program(&src, &FileId::default(), &CompilerOptions::default()).unwrap()
}

// ---------------------------------------------------------------------
// A diagnosed constant-expression initializer must report only its own
// problem. The initializer-fold transform used to be reverted when it
// diagnosed, leaking `SimpleExpr` nodes to later rules and raising a
// P9998 internal error after every legitimate P4037.
// ---------------------------------------------------------------------

#[test]
fn analyze_when_initializer_expression_and_flag_disabled_then_p4037_only() {
    let program = "
FUNCTION func : LREAL
VAR CONSTANT
d2r : LREAL := 3.0/180.0;
END_VAR
func := d2r;
END_FUNCTION";
    let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();

    let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(codes.contains(&"P4037"), "expected P4037, got: {codes:?}");
    // No internal error from a rule observing an unfolded initializer.
    assert!(!codes.contains(&"P9998"), "unexpected P9998 in: {codes:?}");
    // No cascaded "constant must have initializer" — the declaration
    // does carry an initializer, it was merely diagnosed.
    assert!(!codes.contains(&"P4008"), "unexpected P4008 in: {codes:?}");
}

// ---------------------------------------------------------------------
// Don't revert a whole library's type resolution because one unrelated
// declaration failed to resolve.
// ---------------------------------------------------------------------

#[test]
fn analyze_when_unrelated_pou_has_undeclared_type_then_valid_pou_unaffected() {
    let program = "
FUNCTION_BLOCK FB_A
VAR
x : Undeclared_Type;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR
inst : FB_Callee;
END_VAR
inst();
END_FUNCTION_BLOCK
    ";
    let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

    let diagnostics = context.diagnostics();
    assert_eq!(
        1,
        diagnostics.len(),
        "expected exactly one diagnostic, got {diagnostics:?}"
    );
    assert_eq!("P2008", diagnostics[0].code);
}

// ---------------------------------------------------------------------
// Constant-expression VAR initializers.
// ---------------------------------------------------------------------

fn opts_with_constant_initializer_expressions() -> CompilerOptions {
    CompilerOptions {
        allow_constant_initializer_expressions: true,
        // Constants are only collected from true top-level VAR_GLOBAL
        // declarations, which since #1251 (P4028) require this flag.
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn analyze_when_constant_initializer_expression_and_flag_enabled_then_resolves() {
    let program = "
VAR_GLOBAL CONSTANT
SCALE : LREAL := 2.5;
END_VAR
FUNCTION_BLOCK FB_Example
VAR
scaled : LREAL := SCALE*4.0;
END_VAR
END_FUNCTION_BLOCK";
    let lib = parse_program(
        program,
        &FileId::default(),
        &opts_with_constant_initializer_expressions(),
    )
    .unwrap();
    let (_library, context) =
        analyze(&[&lib], &opts_with_constant_initializer_expressions()).unwrap();

    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
}

#[test]
fn analyze_when_constant_initializer_expression_and_flag_disabled_then_diagnostics() {
    let program = "
VAR_GLOBAL CONSTANT
SCALE : LREAL := 2.5;
END_VAR
FUNCTION_BLOCK FB_Example
VAR
scaled : LREAL := SCALE*4.0;
END_VAR
END_FUNCTION_BLOCK";
    let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

    assert!(context.has_diagnostics());
}

// ---------------------------------------------------------------------
// FB-instance call-style initializer (distinct node).
// ---------------------------------------------------------------------

#[test]
fn analyze_when_fb_call_style_init_references_earlier_declared_fb_then_only_not_implemented() {
    // End-to-end: the call-style initializer references an earlier-declared
    // FB. It must produce exactly the "not yet supported" diagnostic
    // (P9999 NotImplemented) from the deferring rule -- and crucially NOT
    // a spurious P2011 "Parent type is not declared", which would appear
    // if the new FunctionBlockCall node were not wired into toposort/type
    // resolution like the FunctionBlock node.
    use ironplc_problems::Problem;

    let program = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
comm : FB_Comm(retries := 3);
END_VAR
END_FUNCTION_BLOCK";
    let lib = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();

    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
    // (must be constructed via Diagnostic::not_implemented), so assert on
    // the stable code string rather than referencing the variant.
    assert!(codes.contains(&"P9999"), "expected P9999, got: {codes:?}");
    assert!(
        !codes.contains(&Problem::ParentTypeNotDeclared.code()),
        "unexpected spurious P2011: {codes:?}"
    );
}

// ---------------------------------------------------------------------
// THIS^ / SUPER^.
// ---------------------------------------------------------------------

/// Diagnostic codes of the whole analysis of `body`, written in a
/// method of `FB_Motor`, which `EXTENDS` `FB_Base`.
fn self_ref_codes(body: &str) -> Vec<String> {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let program = format!(
        "
FUNCTION_BLOCK FB_Base
VAR
level : INT;
END_VAR
METHOD Stop
level := 0;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
VAR
count : INT;
END_VAR
METHOD Start
count := 1;
END_METHOD
METHOD Run
VAR_INPUT
count : INT;
END_VAR
{body}
END_METHOD
END_FUNCTION_BLOCK"
    );
    let lib = parse_program(&program, &FileId::default(), &options).unwrap();
    match analyze(&[&lib], &options) {
        Ok((_library, context)) => context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect(),
        Err(errors) => errors.iter().map(|d| d.code.clone()).collect(),
    }
}

/// Member access and method calls through `THIS^`/`SUPER^` are
/// analyzed: `THIS^.count` is the block's field even where the
/// parameter `count` shadows it, and `SUPER^` reaches the base.
#[rstest::rstest]
#[case::this_field_write("    THIS^.count := count;")]
#[case::this_inherited_field("    THIS^.level := 2;")]
#[case::super_field_read("    THIS^.count := SUPER^.level;")]
#[case::this_method_call("    THIS^.Start();")]
#[case::super_method_call("    SUPER^.Stop();")]
fn analyze_when_self_ref_member_then_ok(#[case] body: &str) {
    assert_eq!(Vec::<String>::new(), self_ref_codes(body));
}

/// Diagnostic codes of the whole analysis of `program`.
fn analysis_codes(program: &str) -> Vec<String> {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    };
    let lib = parse_program(program, &FileId::default(), &options).unwrap();
    match analyze(&[&lib], &options) {
        Ok((_library, context)) => context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect(),
        Err(errors) => errors.iter().map(|d| d.code.clone()).collect(),
    }
}

/// `body`, written in a method of `FB_Leaf`, which `EXTENDS` `FB_Mid`,
/// which `EXTENDS` `FB_Root`. `FB_Mid` overrides `Describe`, with the
/// signature of the method it overrides: XAE rejects an override whose
/// return type differs (`Interface of overridden method 'DESCRIBE' ...
/// doesn't match declaration`), and so does the analysis (P4089). So
/// which override `SUPER^` reaches cannot change the type of a call; the
/// type environment's tests pin that the nearest one is found.
fn leaf_codes(body: &str) -> Vec<String> {
    analysis_codes(&format!(
        "
TYPE Pair : STRUCT f : INT; g : BOOL; END_STRUCT; END_TYPE

FUNCTION_BLOCK FB_Inner
VAR_OUTPUT
out : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Root
VAR
x : INT;
words : ARRAY[1..3] OF INT;
END_VAR
METHOD Describe : INT
Describe := 1;
END_METHOD
METHOD Stop : BOOL
Stop := TRUE;
END_METHOD
METHOD Start
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : INT
Describe := 2;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Leaf EXTENDS FB_Mid
VAR_INPUT
in_val : INT;
END_VAR
VAR_OUTPUT
out_val : INT;
END_VAR
VAR_IN_OUT
io_val : INT;
END_VAR
VAR
s : Pair;
inner : FB_Inner;
n : INT;
flag : BOOL;
END_VAR
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
    ))
}

#[rstest::rstest]
#[case::inherited_method_return_matches("    flag := THIS^.Stop();")]
#[case::grandparent_field("    n := SUPER^.x;")]
#[case::grandparent_method("    THIS^.Start();")]
#[case::nearest_override_through_super("    n := SUPER^.Describe();")]
#[case::nested_struct_member("    n := THIS^.s.f;")]
#[case::nested_instance_output("    n := THIS^.inner.out;")]
#[case::input_output_and_in_out("    THIS^.out_val := THIS^.in_val + THIS^.io_val;")]
#[case::case_insensitive("    this^.N := this^.IN_VAL;")]
fn analyze_when_self_ref_member_of_chain_then_ok(#[case] body: &str) {
    assert_eq!(Vec::<String>::new(), leaf_codes(body));
}

#[rstest::rstest]
#[case::inherited_method_return("    n := THIS^.Stop();", "P4035")]
#[case::nearest_override_through_super("    flag := SUPER^.Describe();", "P4035")]
#[case::nested_struct_member("    flag := THIS^.s.f;", "P4035")]
#[case::nested_instance_output("    flag := THIS^.inner.out;", "P4035")]
#[case::array_element("    flag := THIS^.words[1];", "P4035")]
#[case::unknown_argument("    THIS^.Start(nope := 1);", "P4002")]
#[case::no_return_value("    n := THIS^.Start();", "P4057")]
fn analyze_when_self_ref_misused_then_reported(#[case] body: &str, #[case] code: &str) {
    assert_eq!(vec![code.to_string()], leaf_codes(body));
}

/// Typing `THIS^` must not stop the pass: an expression elsewhere in the
/// same block still gets its type and its error is reported.
#[test]
fn analyze_when_self_ref_and_type_error_then_error_still_reported() {
    let codes = leaf_codes("    THIS^.n := 1;\n    flag := n;");
    assert_eq!(vec!["P4035".to_string()], codes);
}

/// A program has no `THIS^`. Only the rule reports it: the type pass
/// must not add a second diagnostic for the same use.
#[test]
fn analyze_when_self_ref_in_program_then_only_p4074() {
    let codes = analysis_codes(
        "
PROGRAM main
VAR
n : INT;
arr : ARRAY[1..3] OF INT;
END_VAR
n := THIS^.n;
THIS^.arr[2] := 1;
THIS^.Go();
END_PROGRAM",
    );
    assert_eq!(vec!["P4074".to_string(); 3], codes);
}

/// `body`, written in a method `Run` of `FB_Motor`, which `EXTENDS`
/// `FB_Base`. `FB_Base` declares the constant `base_limit` and the
/// variable `base_free`, `FB_Motor` the constant `limit` and the
/// variable `free`. `locals` is the method's own declarations.
fn constant_write_codes(locals: &str, body: &str) -> Vec<String> {
    analysis_codes(&format!(
        "
FUNCTION_BLOCK FB_Base
VAR CONSTANT
base_limit : INT := 1;
END_VAR
VAR
base_free : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
VAR CONSTANT
limit : INT := 1;
END_VAR
VAR
free : INT;
END_VAR
METHOD Run
{locals}
{body}
END_METHOD
END_FUNCTION_BLOCK"
    ))
}

#[rstest::rstest]
#[case::own_constant("", "    THIS^.limit := 2;")]
#[case::inherited_constant("", "    THIS^.base_limit := 2;")]
#[case::inherited_constant_through_super("", "    SUPER^.base_limit := 2;")]
#[case::constant_hidden_by_parameter(
    "VAR_INPUT\n    limit : INT;\nEND_VAR",
    "    THIS^.limit := 2;"
)]
#[case::constant_hidden_by_local("VAR\n    limit : INT;\nEND_VAR", "    THIS^.limit := 2;")]
fn analyze_when_constant_written_through_self_ref_then_p4064(
    #[case] locals: &str,
    #[case] body: &str,
) {
    assert_eq!(
        vec!["P4064".to_string()],
        constant_write_codes(locals, body)
    );
}

#[rstest::rstest]
#[case::own_variable("", "    THIS^.free := 2;")]
#[case::inherited_variable("", "    THIS^.base_free := 2;")]
#[case::inherited_variable_through_super("", "    SUPER^.base_free := 2;")]
#[case::variable_hidden_by_constant_local(
    "VAR CONSTANT\n    free : INT := 1;\nEND_VAR",
    "    THIS^.free := 2;"
)]
#[case::own_field_through_super_is_not_the_constant("", "    SUPER^.base_free := THIS^.limit;")]
fn analyze_when_variable_written_through_self_ref_then_no_p4064(
    #[case] locals: &str,
    #[case] body: &str,
) {
    assert_eq!(Vec::<String>::new(), constant_write_codes(locals, body));
}

// ---------------------------------------------------------------------
// `THIS^`/`SUPER^` cases checked against TwinCAT XAE. Each says what XAE
// reports and what the analysis does. Where they differ, the test pins
// today's behaviour so that fixing it shows up as a failing test; the
// difference is a known gap, not an intended answer.
// ---------------------------------------------------------------------

/// `body` in a method of `FB_A`, which declares an external, a
/// temporary, a property, an instance and a second method.
fn pinned_codes(body: &str) -> Vec<String> {
    analysis_codes(&format!(
        "
VAR_GLOBAL
g : INT;
END_VAR

FUNCTION_BLOCK FB_Inner
VAR_INPUT
i : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A
VAR_EXTERNAL
g : INT;
END_VAR
VAR_TEMP
t : INT;
END_VAR
VAR
inner : FB_Inner;
_p : INT;
END_VAR
PROPERTY Prop : INT
GET
Prop := _p;
END_GET
END_PROPERTY
METHOD Other
VAR
other_local : INT;
END_VAR
END_METHOD
METHOD Run
VAR
n : INT;
flag : BOOL;
END_VAR
{body}
END_METHOD
END_FUNCTION_BLOCK"
    ))
}

/// XAE: `No definition found for base class 'FB_Missing'`. The missing
/// base is reported for itself (P4088), once, whatever the body does
/// with `SUPER^`.
#[rstest::rstest]
#[case::member("    SUPER^.x := 1;", &["P4088"])]
#[case::call("    SUPER^.Go();", &["P4088"])]
fn analyze_when_super_of_undeclared_base_then_base_reported_once(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    let codes = analysis_codes(&format!(
        "
FUNCTION_BLOCK FB_A EXTENDS FB_Missing
METHOD Run
{body}
END_METHOD
END_FUNCTION_BLOCK"
    ));
    assert_eq!(expected, codes.as_slice());
}

/// XAE accepts `THIS^.t` of a `VAR_TEMP`, and so does the analysis,
/// typing it. XAE rejects `THIS^.g` of a `VAR_EXTERNAL` (`'g' is no
/// component of ...`). Gap: the analysis does not type it, but it does
/// not report it either, so a wrong-typed assignment is not caught.
#[rstest::rstest]
#[case::external_in_range("    n := THIS^.g;", &[])]
#[case::external_wrong_type("    flag := THIS^.g;", &[])]
#[case::temporary_in_range("    n := THIS^.t;", &[])]
#[case::temporary_wrong_type("    flag := THIS^.t;", &["P4035"])]
fn analyze_when_self_ref_names_external_or_temporary_then_pinned(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(expected, pinned_codes(body).as_slice());
}

/// XAE types `THIS^.Prop` (`Cannot convert type 'INT' to type 'BOOL'`),
/// so a wrong-typed assignment from it is P4035, and an assignment of the
/// property's own type is accepted.
#[rstest::rstest]
#[case::property_wrong_type("    flag := THIS^.Prop;", &["P4035"])]
#[case::property_own_type("    n := THIS^.Prop;", &[])]
fn analyze_when_self_ref_names_a_property_then_typed(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(expected, pinned_codes(body).as_slice());
}

/// A property the block inherits is typed through `THIS^` and `SUPER^`.
#[rstest::rstest]
#[case::this_wrong_type("flag := THIS^.Prop;", &["P4035"])]
#[case::super_wrong_type("flag := SUPER^.Prop;", &["P4035"])]
#[case::this_own_type("n := THIS^.Prop;", &[])]
#[case::super_own_type("n := SUPER^.Prop;", &[])]
fn analyze_when_self_ref_names_an_inherited_property_then_typed(
    #[case] body: &str,
    #[case] expected: &[&str],
) {
    let codes = analysis_codes(&format!(
        "
FUNCTION_BLOCK FB_Base
VAR
_p : INT;
END_VAR
PROPERTY Prop : INT
GET
Prop := _p;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Run
VAR
n : INT;
flag : BOOL;
END_VAR
{body}
END_METHOD
END_FUNCTION_BLOCK"
    ));
    assert_eq!(expected, codes.as_slice());
}

/// XAE rejects `THIS^.other_local` (`'other_local' is no component of
/// ...`) and `THIS^.Other` without a call (`Cannot convert type 'OTHER'
/// to type 'INT'`). Gap: the analysis reports neither and the access has
/// no type, so a wrong-typed assignment is not caught.
#[rstest::rstest]
#[case::local_of_another_method("    flag := THIS^.other_local;")]
#[case::method_name_without_call("    flag := THIS^.Other;")]
fn analyze_when_self_ref_names_a_non_field_then_pinned_untyped_and_unreported(#[case] body: &str) {
    assert_eq!(Vec::<String>::new(), pinned_codes(body));
}

/// XAE accepts `THIS^.inner(i := 1)`, so the analysis does too. The
/// arguments are not checked against the instance's inputs.
#[test]
fn analyze_when_member_instance_called_through_self_ref_then_ok() {
    assert_eq!(
        Vec::<String>::new(),
        pinned_codes("    THIS^.inner(i := 1);")
    );
}

/// XAE accepts `inst.inner(i := 1)` where `inst` is an instance of a
/// block that declares the function block instance `inner`, from a
/// program and from another block. A name that is neither a method nor a
/// function block instance is still an undeclared method.
#[rstest::rstest]
#[case::from_program(
    "PROGRAM main\nVAR o : FB_Outer; END_VAR\n    o.inner(i := 1);\nEND_PROGRAM",
    &[]
)]
#[case::from_block(
    "FUNCTION_BLOCK FB_User\nVAR o : FB_Outer; END_VAR\nMETHOD Run\n    o.inner(i := 1);\nEND_METHOD\nEND_FUNCTION_BLOCK",
    &[]
)]
#[case::inherited_instance(
    "PROGRAM main\nVAR d : FB_Derived; END_VAR\n    d.inner(i := 1);\nEND_PROGRAM",
    &[]
)]
#[case::undeclared_name(
    "PROGRAM main\nVAR o : FB_Outer; END_VAR\n    o.bogus(i := 1);\nEND_PROGRAM",
    &["P4046"]
)]
#[case::member_that_is_not_an_instance(
    "PROGRAM main\nVAR o : FB_Outer; END_VAR\n    o.count(i := 1);\nEND_PROGRAM",
    &["P4046"]
)]
fn analyze_when_member_instance_called_through_named_instance_then_xae(
    #[case] user: &str,
    #[case] expected: &[&str],
) {
    let codes = analysis_codes(&format!(
        "
FUNCTION_BLOCK FB_Inner
VAR_INPUT
i : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Outer
VAR
inner : FB_Inner;
count : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Outer
END_FUNCTION_BLOCK

{user}"
    ));
    assert_eq!(expected, codes.as_slice());
}

/// A bit access through `THIS^` in a program is reported once, as P4074.
#[test]
fn analyze_when_bit_access_through_self_ref_in_program_then_only_p4074() {
    let codes = analysis_codes(
        "
PROGRAM main
VAR
flags : WORD;
END_VAR
THIS^.flags.0 := TRUE;
END_PROGRAM",
    );
    assert_eq!(vec!["P4074".to_string()], codes);
}

/// A bare `THIS^`, used as a value on its own, is not analyzed yet.
#[test]
fn analyze_when_bare_self_ref_then_not_implemented() {
    let codes = self_ref_codes("    THIS^ := THIS^;");
    assert!(codes.contains(&"P9999".to_string()), "{codes:?}");
}

/// The same function block without `THIS^` analyzes cleanly -- the new
/// arms must not report anything for programs that do not use it.
#[test]
fn analyze_when_no_self_ref_then_no_not_implemented() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let program = "
FUNCTION_BLOCK FB_Motor
VAR
count : INT;
END_VAR
METHOD Start
count := 1;
END_METHOD
END_FUNCTION_BLOCK";
    let lib = parse_program(program, &FileId::default(), &options).unwrap();
    let (_library, context) = analyze(&[&lib], &options).unwrap();

    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(codes.is_empty(), "expected no diagnostics, got: {codes:?}");
}

// ---------------------------------------------------------------------
// The call hierarchy's other direction: a program is not a type, so no
// POU can declare an instance of one or invoke one. Nothing enforces
// this in a rule; it falls out of a program not being in the type
// environment, which makes it the pipeline's behaviour to pin.
// ---------------------------------------------------------------------

/// The codes reported for a POU that names `Target`, a program, as the
/// type of a variable and then invokes that variable.
fn codes_for_pou_referencing_a_program(pou: &str) -> Vec<String> {
    let program = format!(
        "
PROGRAM Target
VAR
x : INT;
END_VAR
x := 1;
END_PROGRAM

{pou}"
    );
    let lib = parse_program(&program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_library, context) = analyze(&[&lib], &CompilerOptions::default()).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

#[test]
fn analyze_when_function_block_declares_program_instance_then_undeclared_type() {
    let codes = codes_for_pou_referencing_a_program(
        "
FUNCTION_BLOCK Caller
VAR
p : Target;
END_VAR
p();
END_FUNCTION_BLOCK",
    );
    assert_eq!(vec!["P2008", "P4012"], codes);
}

#[test]
fn analyze_when_function_declares_program_instance_then_undeclared_type() {
    let codes = codes_for_pou_referencing_a_program(
        "
FUNCTION Caller : BOOL
VAR
p : Target;
END_VAR
p();
Caller := TRUE;
END_FUNCTION",
    );
    assert_eq!(vec!["P2008", "P4012"], codes);
}

// A program may not invoke a program either; only a resource instantiates
// one.
#[test]
fn analyze_when_program_declares_program_instance_then_undeclared_type() {
    let codes = codes_for_pou_referencing_a_program(
        "
PROGRAM Caller
VAR
p : Target;
END_VAR
p();
END_PROGRAM",
    );
    assert_eq!(vec!["P2008", "P4012"], codes);
}

#[test]
fn analyze_when_function_block_invokes_program_by_name_then_not_in_scope() {
    let codes = codes_for_pou_referencing_a_program(
        "
FUNCTION_BLOCK Caller
VAR
y : INT;
END_VAR
Target();
y := 2;
END_FUNCTION_BLOCK",
    );
    assert_eq!(vec!["P4012"], codes);
}

// The one legitimate way to instantiate a program stays legitimate.
#[test]
fn analyze_when_resource_instantiates_program_then_ok() {
    let codes = codes_for_pou_referencing_a_program(
        "
CONFIGURATION Config
RESOURCE Res ON PLC
TASK T(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM Inst WITH T : Target;
END_RESOURCE
END_CONFIGURATION",
    );
    assert!(codes.is_empty(), "expected no diagnostics, got: {codes:?}");
}

fn variable_type_id(
    context: &crate::semantic_context::SemanticContext,
    name: &str,
    scope: &crate::symbol_environment::ScopeKind,
) -> Option<ironplc_dsl::type_id::TypeId> {
    context
        .symbols()
        .find(&ironplc_dsl::core::Id::from(name), scope)
        .expect("variable symbol")
        .type_id
}

#[test]
fn resolve_types_when_variables_declared_then_symbols_record_type_id() {
    use crate::symbol_environment::{ScopeKind, ScopePath};
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::Id;

    let (_library, context) = parse_and_resolve_types_with_context(
        "
TYPE Point : STRUCT x : INT; END_STRUCT; END_TYPE
PROGRAM main
  VAR
count : INT;
p : Point;
arr : ARRAY[1..2] OF DINT;
  END_VAR
END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL
flag : BOOL;
  END_VAR
  RESOURCE res ON PLC
TASK t(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION
",
    );
    let types = context.types();
    let main = ScopeKind::Named(ScopePath::new(vec![Id::from("main")]));

    assert_eq!(
        variable_type_id(&context, "count", &main),
        types.id_of(&TypeName::from("INT"))
    );
    assert_eq!(
        variable_type_id(&context, "p", &main),
        types.id_of(&TypeName::from("Point"))
    );
    let arr = variable_type_id(&context, "arr", &main).expect("anonymous array type id");
    assert!(types.get_by_id(arr).unwrap().representation.is_array());
    assert_eq!(
        variable_type_id(&context, "flag", &ScopeKind::Global),
        types.id_of(&TypeName::from("BOOL"))
    );
}

#[test]
fn resolve_types_when_system_uptime_global_then_symbol_records_type_id() {
    use crate::symbol_environment::ScopeKind;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_dsl::common::TypeName;

    let options = CompilerOptions {
        allow_system_uptime_global: true,
        ..CompilerOptions::default()
    };
    let (_library, context) = parse_and_resolve_types_with_options(
        "
PROGRAM main
END_PROGRAM
",
        &options,
    );

    assert_eq!(
        variable_type_id(&context, "__SYSTEM_UP_LTIME", &ScopeKind::Global),
        context.types().id_of(&TypeName::from("LTIME"))
    );
}
