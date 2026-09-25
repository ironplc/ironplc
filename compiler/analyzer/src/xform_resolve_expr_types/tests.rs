//! Unit tests for `xform_resolve_expr_types`.

use super::apply;
use crate::type_environment::TypeEnvironmentBuilder;
use crate::xform_resolve_late_bound_expr_kind;
use crate::xform_resolve_symbol_and_function_environment;
use crate::xform_resolve_type_decl_environment;
use crate::{
    function_environment::FunctionEnvironmentBuilder, symbol_environment::SymbolEnvironment,
};
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

mod single_assignment;

/// Runs the prerequisite passes and then the expression type resolution pass.
fn run_pass(program: &str) -> Library {
    run_pass_with_options(program, &CompilerOptions::default())
}

/// Like [`run_pass`] but with explicit compiler options (needed for REF_TO tests).
fn run_pass_with_options(program: &str, options: &CompilerOptions) -> Library {
    let library = ironplc_parser::parse_program(program, &FileId::default(), options).unwrap();
    let mut type_environment = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .with_stdlib_function_blocks()
        .build()
        .unwrap();
    let library = xform_resolve_type_decl_environment::apply(library, &mut type_environment)
        .unwrap()
        .0;
    let library = xform_resolve_late_bound_expr_kind::apply(library, &mut type_environment)
        .unwrap()
        .0;
    let mut function_environment = FunctionEnvironmentBuilder::new()
        .with_stdlib_functions()
        .build();
    let mut symbol_environment = SymbolEnvironment::new();
    let (library, _diagnostics) = xform_resolve_symbol_and_function_environment::apply(
        library,
        &mut symbol_environment,
        &mut function_environment,
    )
    .unwrap();
    apply(
        library,
        &mut type_environment,
        &function_environment,
        options,
    )
    .unwrap()
}

/// Helper visitor to collect resolved types from assignment RHS expressions.
struct ResolvedTypeCollector {
    types: Vec<Option<ironplc_dsl::common::TypeName>>,
}

impl ResolvedTypeCollector {
    fn new() -> Self {
        Self { types: vec![] }
    }
}

impl Fold<()> for ResolvedTypeCollector {
    fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, ()> {
        self.types.push(node.value.resolved_type.clone());
        node.recurse_fold(self)
    }
}

/// Collects the resolved_type from the top-level assignment expressions.
fn collect_assignment_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
    let mut collector = ResolvedTypeCollector::new();
    let _ = collector.fold_library(library.clone());
    collector.types
}

/// Collects resolved_type from every Expr node in the tree.
struct AllExprTypeCollector {
    types: Vec<Option<ironplc_dsl::common::TypeName>>,
}

impl AllExprTypeCollector {
    fn new() -> Self {
        Self { types: vec![] }
    }
}

impl Fold<()> for AllExprTypeCollector {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
        self.types.push(node.resolved_type.clone());
        node.recurse_fold(self)
    }
}

fn collect_all_expr_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
    let mut collector = AllExprTypeCollector::new();
    let _ = collector.fold_library(library.clone());
    collector.types
}

/// Returns the resolved type name as an uppercase &str for comparison.
/// TypeEnvironment stores elementary types in lowercase, but IEC 61131-3
/// type names are case-insensitive, so we normalize to uppercase for assertions.
fn type_name_upper(tn: &Option<ironplc_dsl::common::TypeName>) -> Option<String> {
    tn.as_ref().map(|t| t.name.original().to_uppercase())
}

fn assert_type_eq(tn: &Option<ironplc_dsl::common::TypeName>, expected: &str) {
    assert_eq!(
        type_name_upper(tn),
        Some(expected.to_string()),
        "Expected type {expected}"
    );
}

#[test]
fn apply_when_nested_arithmetic_then_all_subexprs_resolve() {
    // (x + y) * z — the inner (x + y) and outer multiply should all resolve to INT
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    result : INT;
END_VAR
    result := (x + y) * z;
END_FUNCTION_BLOCK";

    let result = run_pass(program);
    let types = collect_all_expr_types(&result);
    // Every expression node in (x + y) * z should resolve to INT:
    // nodes: result:=(expr), (x+y)*z, (x+y), x, y, z — all INT
    for (i, t) in types.iter().enumerate() {
        assert!(
            t.is_some(),
            "Expression node {i} should have a resolved type, got None"
        );
        assert_eq!(
            type_name_upper(t),
            Some("INT".to_string()),
            "Expression node {i} should be INT"
        );
    }
}

#[test]
fn apply_when_nested_comparison_with_arithmetic_then_resolves_correctly() {
    // (x + y) > z — inner (x + y) is INT, the comparison is BOOL
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    flag : BOOL;
END_VAR
    flag := (x + y) > z;
END_FUNCTION_BLOCK";

    let result = run_pass(program);
    let top_types = collect_assignment_types(&result);
    assert_eq!(top_types.len(), 1);
    assert_type_eq(&top_types[0], "BOOL");

    // Also verify inner nodes
    let all_types = collect_all_expr_types(&result);
    // The top-level expr is BOOL (comparison), but inner operands should be INT
    let has_bool = all_types
        .iter()
        .any(|t| type_name_upper(t) == Some("BOOL".to_string()));
    let has_int = all_types
        .iter()
        .any(|t| type_name_upper(t) == Some("INT".to_string()));
    assert!(has_bool, "Should have BOOL from comparison");
    assert!(has_int, "Should have INT from arithmetic operands");
}

#[test]
fn apply_when_negated_nested_expr_then_resolves_type() {
    // -(x + y) — unary negation of a parenthesized binary op
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    result : INT;
END_VAR
    result := -(x + y);
END_FUNCTION_BLOCK";

    let result = run_pass(program);
    let top_types = collect_assignment_types(&result);
    assert_eq!(top_types.len(), 1);
    assert_type_eq(&top_types[0], "INT");

    let all_types = collect_all_expr_types(&result);
    // All nodes (unary, parenthesized, binary, x, y) should be INT
    for (i, t) in all_types.iter().enumerate() {
        assert!(
            t.is_some(),
            "Expression node {i} should have a resolved type"
        );
        assert_eq!(
            type_name_upper(t),
            Some("INT".to_string()),
            "Expression node {i} should be INT"
        );
    }
}

#[test]
fn apply_when_deeply_nested_parens_then_resolves_type() {
    // ((x)) — multiple levels of parenthesization
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ((x));
END_FUNCTION_BLOCK";

    let result = run_pass(program);
    let top_types = collect_assignment_types(&result);
    assert_eq!(top_types.len(), 1);
    assert_type_eq(&top_types[0], "INT");

    let all_types = collect_all_expr_types(&result);
    for (i, t) in all_types.iter().enumerate() {
        assert_eq!(
            type_name_upper(t),
            Some("INT".to_string()),
            "Expression node {i} should be INT"
        );
    }
}

#[test]
fn apply_when_multiple_assignments_then_each_resolves_independently() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
    a : INT;
    b : BOOL;
END_VAR
    a := x;
    b := y;
END_FUNCTION_BLOCK";

    let result = run_pass(program);
    let types = collect_assignment_types(&result);
    assert_eq!(types.len(), 2);
    assert_type_eq(&types[0], "INT");
    assert_type_eq(&types[1], "BOOL");
}

#[test]
fn apply_when_function_return_var_used_in_builtin_then_resolves_type() {
    let program = "
FUNCTION FOO : INT
  VAR_INPUT
    A : INT;
  END_VAR
  FOO := 8;
  FOO := SHR(FOO, 1);
END_FUNCTION

PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := FOO(A := 5);
END_PROGRAM";

    let result = run_pass(program);
    let all_types = collect_all_expr_types(&result);
    // Every expression node should have a resolved type (no None values)
    for (i, t) in all_types.iter().enumerate() {
        assert!(
            t.is_some(),
            "Expression node {i} should have a resolved type, got None"
        );
    }
}

#[test]
fn apply_when_ref_to_inline_array_deref_subscript_then_resolves_element_type() {
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    let program = "
FUNCTION GET_CHAR_BYTE : BYTE
VAR_INPUT
    pt : REF_TO ARRAY[1..255] OF BYTE;
    pos : INT;
END_VAR
    GET_CHAR_BYTE := pt^[pos];
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
END_VAR
    result := GET_CHAR_BYTE(pt := NULL, pos := 1);
END_PROGRAM";

    let result = run_pass_with_options(program, &options);
    let types = collect_assignment_types(&result);
    // First assignment: GET_CHAR_BYTE := pt^[pos] — should resolve to BYTE
    assert_type_eq(&types[0], "BYTE");
}

// -----------------------------------------------------------------
// AND_THEN / OR_ELSE short-circuit boolean operators.
// See specs/design/beckhoff-twincat-dialect.md §3.4.
// -----------------------------------------------------------------

#[test]
fn apply_when_or_else_used_then_resolves_like_or() {
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    let program = "
FUNCTION_BLOCK FB_Example
VAR
    a : BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
    result := a OR_ELSE b;
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &options);
    let types = collect_assignment_types(&result);
    assert_type_eq(&types[0], "BOOL");
}

#[test]
fn apply_when_and_then_used_then_resolves_like_and() {
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    let program = "
FUNCTION_BLOCK FB_Example
VAR
    a : BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
    result := a AND_THEN b;
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &options);
    let types = collect_assignment_types(&result);
    assert_type_eq(&types[0], "BOOL");
}

// -----------------------------------------------------------------
// EXTENDS field inheritance.
// -----------------------------------------------------------------

#[test]
fn apply_when_expression_uses_inherited_field_then_resolves_type() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let program = "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
    bRunning := bEnabled AND bRunning;
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &options);
    let types = collect_assignment_types(&result);
    assert_type_eq(&types[0], "BOOL");
}

#[test]
fn apply_when_function_block_output_in_condition_then_resolves_type() {
    // An IF condition has no assignment target to borrow a type from, so
    // codegen reads the condition's own resolved_type. Leaving it unset
    // for `timer.Q` produced P9999 (issue #1375).
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    timer : TON;
    done : BOOL;
END_VAR
    IF timer.Q THEN
        done := TRUE;
    END_IF;
END_FUNCTION_BLOCK";
    let result = run_pass(program);
    let types = collect_all_expr_types(&result);
    assert!(
        types
            .iter()
            .any(|t| type_name_upper(t).as_deref() == Some("BOOL")),
        "condition `timer.Q` should resolve to BOOL, got {types:?}"
    );
    assert!(
        types.iter().all(|t| t.is_some()),
        "every expression should resolve, got {types:?}"
    );
}

#[test]
fn apply_when_unknown_function_block_field_then_type_is_unresolved() {
    // A misspelled member has no field to resolve against; the pass must
    // leave it unset rather than inventing a type.
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    timer : TON;
    result : BOOL;
END_VAR
    result := timer.NOT_A_FIELD;
END_FUNCTION_BLOCK";
    let result = run_pass(program);
    let types = collect_assignment_types(&result);
    assert_eq!(types.len(), 1);
    assert_eq!(types[0], None);
}

// ---------------------------------------------------------------------
// METHOD scoping.
// See https://github.com/ironplc/ironplc/issues/1439.
// ---------------------------------------------------------------------

fn opts_with_fb_inheritance() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// Before a method opened a scope this pass never saw a method's
/// variables at all, so every reference in a method body resolved to
/// `None` and every type rule downstream skipped it silently.
#[test]
fn apply_when_method_local_then_resolves_type() {
    let program = "
FUNCTION_BLOCK FB_TEST
METHOD m
VAR
    x : INT;
    y : INT;
END_VAR
    x := y;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 1);
    assert_eq!(types[0].as_ref().map(|t| t.to_string()), Some("int".into()));
}

/// The method scope nests inside the function block's, so a method
/// local shadows a field of the same name for that method only.
#[test]
fn apply_when_method_local_shadows_field_then_resolves_local_type() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    v : INT;
    target : REAL;
END_VAR
METHOD m
VAR
    v : REAL;
END_VAR
    target := v;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 1);
    assert_eq!(
        types[0].as_ref().map(|t| t.to_string()),
        Some("real".into()),
        "the method's own `v` should shadow the function block's"
    );
}

/// Each method is its own scope, so the same name in two methods is
/// two variables and each resolves to its own declared type.
#[test]
fn apply_when_two_methods_declare_same_name_then_each_resolves_own_type() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    i : INT;
    r : REAL;
END_VAR
METHOD a
VAR
    q : INT;
END_VAR
    i := q;
END_METHOD
METHOD b
VAR
    q : REAL;
END_VAR
    r := q;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 2);
    assert_eq!(types[0].as_ref().map(|t| t.to_string()), Some("int".into()));
    assert_eq!(
        types[1].as_ref().map(|t| t.to_string()),
        Some("real".into())
    );
}

/// A method that declares a return type has a result variable of that
/// type, the same way a function does, so reading the method's own
/// name inside its body resolves.
#[test]
fn apply_when_method_reads_own_name_then_resolves_return_type() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    nLast : DINT;
END_VAR
METHOD GetSpeed : DINT
    GetSpeed := 1;
    nLast := GetSpeed;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 2);
    assert_eq!(
        types[1].as_ref().map(|t| t.to_string()),
        Some("dint".into()),
        "the method's own name should resolve to its return type"
    );
}

/// A method with no return type has no result variable, so its name
/// is not a variable and resolves to nothing. The analyzer rejects
/// the reference outright; this pins the same rule in this pass.
#[test]
fn apply_when_method_has_no_return_type_then_own_name_unresolved() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    nLast : DINT;
END_VAR
METHOD DoThing
    nLast := DoThing;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 1);
    assert_eq!(types[0], None);
}

/// A method's locals leave with the method: the enclosing function
/// block's body must not resolve them.
#[test]
fn apply_when_function_block_body_references_method_local_then_unresolved() {
    let program = "
FUNCTION_BLOCK FB_TEST
VAR
    target : INT;
END_VAR
    target := q;
METHOD m
VAR
    q : INT;
END_VAR
    q := 1;
END_METHOD
END_FUNCTION_BLOCK";

    let result = run_pass_with_options(program, &opts_with_fb_inheritance());
    let types = collect_assignment_types(&result);

    assert_eq!(types.len(), 2);
    assert_eq!(
        types[0], None,
        "the function block body must not see the method's local"
    );
}
