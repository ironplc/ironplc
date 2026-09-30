//! Unit tests for `xform_fold_initializer_expressions`.
use super::*;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_test::cast;

use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};

fn opts() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Rusty)
}

/// The elementary types only: the unit tests below declare no types, and
/// the named-constant type check skips a type it cannot resolve.
fn types() -> TypeEnvironment {
    TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap()
}

fn parse(src: &str, options: &CompilerOptions) -> Library {
    parse_program(src, &FileId::default(), options).unwrap()
}

fn find_var_decl<'a>(lib: &'a Library, var_name: &str) -> &'a VarDecl {
    for element in &lib.elements {
        let vars = match element {
            LibraryElementKind::FunctionBlockDeclaration(fb) => &fb.variables,
            LibraryElementKind::FunctionDeclaration(f) => &f.variables,
            LibraryElementKind::ProgramDeclaration(p) => &p.variables,
            LibraryElementKind::GlobalVarDeclarations(decls) => decls,
            _ => continue,
        };
        for var in vars {
            if var.identifier.to_string().eq_ignore_ascii_case(var_name) {
                return var;
            }
        }
    }
    panic!("Variable '{}' not found", var_name);
}

/// Applies the transform expecting no diagnostics; returns the library.
fn apply_clean(lib: Library, options: &CompilerOptions) -> Library {
    let (lib, diagnostics) = apply(lib, &types(), options).unwrap();
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    lib
}

/// Applies the transform expecting diagnostics; returns them.
fn apply_expect_diagnostics(lib: Library, options: &CompilerOptions) -> Vec<Diagnostic> {
    let (_, diagnostics) = apply(lib, &types(), options).unwrap();
    assert!(!diagnostics.is_empty(), "expected diagnostics");
    diagnostics
}

fn real_value(var: &VarDecl) -> f64 {
    let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::RealLiteral
    );
    lit.value
}

#[test]
fn apply_when_arithmetic_initializer_then_folds_to_literal() {
    let lib = parse(
        "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
}

#[test]
fn apply_when_named_constant_initializer_then_substitutes_and_folds() {
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            PI : LREAL := 4.25;
        END_VAR
        PROGRAM main
        VAR
            d2r : LREAL := PI/180.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
}

#[test]
fn apply_when_nested_arithmetic_then_folds_completely() {
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            PI : LREAL := 4.25;
        END_VAR
        PROGRAM main
        VAR
            asec2r : LREAL := PI/(180.0*3600.0);
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "asec2r");
    assert!((real_value(var) - (4.25 / (180.0 * 3600.0))).abs() < f64::EPSILON);
}

/// A repeated global constant keeps its first value here; the repeat is
/// the symbol environment's to report, so this pass stays clean.
#[test]
fn apply_when_duplicate_global_constant_then_first_value_kept() {
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            SCALE : LREAL := 2.0;
        END_VAR
        VAR_GLOBAL CONSTANT
            SCALE : LREAL := 3.0;
        END_VAR
        PROGRAM main
        VAR
            d2r : LREAL := SCALE*180.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - 360.0).abs() < f64::EPSILON);
}

#[test]
fn apply_when_constant_reference_different_case_then_resolves() {
    // Constant lookup is case-insensitive, matching Id's own
    // case-insensitive Hash/Eq (per the IEC 61131-3 spec) -- reusing
    // Id as the table key gets this for free, no manual
    // to_uppercase() needed.
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            scale : LREAL := 2.0;
        END_VAR
        PROGRAM main
        VAR
            d2r : LREAL := SCALE*180.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - (2.0 * 180.0)).abs() < f64::EPSILON);
}

#[test]
fn apply_when_reference_to_non_constant_then_error() {
    let lib = parse(
        "
        PROGRAM main
        VAR
            scale : LREAL := 2.0;
            d2r : LREAL := scale/180.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
}

#[test]
fn apply_when_flag_disabled_then_error_even_if_foldable() {
    let lib = parse(
        "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM",
        &opts(),
    );
    let (lib, diagnostics) = apply(lib, &types(), &CompilerOptions::default()).unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d.code == Problem::ConstantInitializerExpressionNotAllowed.code()));
    // The initializer is still folded best-effort so that a `VAR
    // CONSTANT` declaration is not additionally (and misleadingly)
    // diagnosed as uninitialized by a downstream rule.
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
}

#[test]
fn apply_when_bare_literal_initializer_then_unchanged() {
    let lib = parse(
        "PROGRAM main VAR x : LREAL := 4.25; END_VAR END_PROGRAM",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "x");
    assert!((real_value(var) - 4.25).abs() < f64::EPSILON);
}

#[test]
fn apply_when_function_local_constant_then_resolves() {
    let lib = parse(
        "
        FUNCTION my_func : LREAL
        VAR CONSTANT
            SCALE : LREAL := 2.0;
        END_VAR
        VAR
            d2r : LREAL := SCALE*180.0;
        END_VAR
        my_func := d2r;
        END_FUNCTION
    ",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "d2r");
    assert!((real_value(var) - (2.0 * 180.0)).abs() < f64::EPSILON);
}

#[test]
fn apply_when_fb_local_constant_not_visible_in_other_fb_then_error() {
    let lib = parse(
        "
        FUNCTION_BLOCK fb1
        VAR CONSTANT
            LOCAL_SCALE : LREAL := 2.0;
        END_VAR
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK fb2
        VAR
            d2r : LREAL := LOCAL_SCALE*180.0;
        END_VAR
        END_FUNCTION_BLOCK
    ",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
}

#[test]
fn apply_when_mutual_reference_cycle_then_error_not_hang() {
    // A references B and B references A. `constants` only ever admits
    // already-literal values (collected before any substitution runs),
    // so neither A nor B is ever registered as a known constant here --
    // both fail to fold, and a downstream reference to either also
    // fails. This is a regression test proving that shape terminates
    // with diagnostics rather than recursing indefinitely.
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            A : LREAL := B+0.0;
            B : LREAL := A+0.0;
        END_VAR
        PROGRAM main
        VAR
            x : LREAL := A+0.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn apply_when_self_reference_cycle_then_error_not_hang() {
    // A constant whose initializer references itself (`A := A + 0.0`).
    // This is the degenerate one-node cycle. `A` is never registered as
    // a known constant (only bare-literal `Simple` initializers are), so
    // the self-reference does not resolve, the expression does not fold,
    // and a P4037 diagnostic is emitted. The test completing at all is
    // the proof of termination -- there is no bounded recursion here
    // because substitution can only ever replace a name with a literal,
    // never with another name-bearing expression.
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            A : LREAL := A+0.0;
        END_VAR
        PROGRAM main
        VAR
            x : LREAL := A+0.0;
        END_VAR
        END_PROGRAM
    ",
        &opts(),
    );
    let errors = apply_expect_diagnostics(lib, &opts());
    // One diagnostic for A's own initializer, one for x referencing A.
    assert_eq!(errors.len(), 2);
    assert!(errors
        .iter()
        .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
}

#[test]
fn apply_when_three_node_cycle_then_error_not_hang() {
    // A longer cycle A -> B -> C -> A. Same reasoning as the mutual
    // (two-node) case: none of A, B, C is a bare literal, so none is
    // registered, every initializer fails to fold, and each emits a
    // diagnostic. Demonstrates termination is independent of cycle
    // length -- the pass never chases references transitively.
    let lib = parse(
        "
        VAR_GLOBAL CONSTANT
            A : LREAL := B+0.0;
            B : LREAL := C+0.0;
            C : LREAL := A+0.0;
        END_VAR
    ",
        &opts(),
    );
    let errors = apply_expect_diagnostics(lib, &opts());
    assert_eq!(errors.len(), 3);
    assert!(errors
        .iter()
        .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
}

#[test]
fn apply_when_integer_arithmetic_then_folds_to_integer_literal() {
    let lib = parse(
        "PROGRAM main VAR x : DINT := 2+3; END_VAR END_PROGRAM",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    let var = find_var_decl(&lib, "x");
    let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::IntegerLiteral
    );
    assert_eq!(lit.value.value.value, 5);
}

#[test]
fn apply_when_initializer_divides_by_zero_then_division_by_zero_error_not_misleading_p4038() {
    // 1.0/0.0 is a genuine constant expression (both operands known);
    // it must be reported as "division by zero", not misdiagnosed as
    // "not a constant expression" (P4038 / InitializerNotConstantExpression).
    let lib = parse(
        "PROGRAM main VAR x : LREAL := 1.0/0.0; END_VAR END_PROGRAM",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::ConstantExpressionDivisionByZero.code()));
}

#[test]
fn apply_when_initializer_int_overflows_then_overflow_error_not_misleading_p4038() {
    let lib = parse(
        "PROGRAM main VAR x : LINT := 170141183460469231731687303715884105727 * 2; END_VAR END_PROGRAM",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::ConstantExpressionOverflow.code()));
}

#[test]
fn apply_when_initializer_real_overflows_then_overflow_error_not_misleading_p4038() {
    let lib = parse(
        "PROGRAM main VAR x : LREAL := 1.0E300 * 1.0E300; END_VAR END_PROGRAM",
        &opts(),
    );
    let diagnostics = apply_expect_diagnostics(lib, &opts());
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::ConstantExpressionOverflow.code()));
}

/// A method's `VAR CONSTANT` belongs to that method. Before the
/// method scope existed every method's constants were registered in
/// the enclosing function block's scope, so a sibling could fold
/// against them. See
/// https://github.com/ironplc/ironplc/issues/1439.
#[test]
fn apply_when_method_local_constant_not_visible_in_sibling_method_then_error() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..opts()
    };
    let lib = parse(
        "
        FUNCTION_BLOCK fb1
        VAR
            x : LREAL;
        END_VAR
        METHOD m1
        VAR CONSTANT
            LOCAL_SCALE : LREAL := 2.0;
        END_VAR
            x := 1.0;
        END_METHOD
        METHOD m2
        VAR
            d2r : LREAL := LOCAL_SCALE*180.0;
        END_VAR
            x := 2.0;
        END_METHOD
        END_FUNCTION_BLOCK
    ",
        &options,
    );
    let diagnostics = apply_expect_diagnostics(lib, &options);
    assert!(diagnostics
        .iter()
        .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
}

fn integer_value(var: &VarDecl) -> u128 {
    let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::IntegerLiteral
    );
    lit.value.value.value
}

#[test]
fn apply_when_bare_global_constant_name_then_folds_to_constant_value() {
    let lib = parse(
        "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
FUNCTION_BLOCK FB_A
VAR
    x : UDINT := C;
END_VAR
END_FUNCTION_BLOCK",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    assert_eq!(7, integer_value(find_var_decl(&lib, "x")));
}

#[test]
fn apply_when_bare_local_constant_name_then_folds_to_constant_value() {
    let lib = parse(
        "
PROGRAM main
VAR CONSTANT
    L : UDINT := 3;
END_VAR
VAR
    x : UDINT := L;
END_VAR
END_PROGRAM",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    assert_eq!(3, integer_value(find_var_decl(&lib, "x")));
}

#[test]
fn apply_when_constant_initialized_from_constant_then_both_fold() {
    let lib = parse(
        "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
    D : UDINT := C;
    E : UDINT := D + 1;
END_VAR
PROGRAM main
VAR
    x : UDINT := D;
    y : UDINT := E;
END_VAR
END_PROGRAM",
        &opts(),
    );
    let lib = apply_clean(lib, &opts());
    assert_eq!(7, integer_value(find_var_decl(&lib, "x")));
    assert_eq!(8, integer_value(find_var_decl(&lib, "y")));
}

#[test]
fn apply_when_bare_constant_name_then_value_located_at_reference() {
    // A problem with the value, such as a range check, belongs where the
    // constant is used, not where it is declared.
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
PROGRAM main
VAR
    x : UDINT := C;
END_VAR
END_PROGRAM";
    let lib = apply_clean(parse(source, &opts()), &opts());
    let simple = cast!(
        &find_var_decl(&lib, "x").initializer,
        InitialValueAssignmentKind::Simple
    );
    let reference = source.rfind(":= C").unwrap() + 3;
    assert_eq!(
        reference,
        simple.initial_value.as_ref().unwrap().span().start
    );
}

#[test]
fn apply_when_bare_constant_name_and_flag_disabled_then_p4037() {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
PROGRAM main
VAR
    x : UDINT := C;
END_VAR
END_PROGRAM";
    let options = CompilerOptions::default();
    let diagnostics = apply_expect_diagnostics(parse(source, &options), &options);
    assert_eq!(1, diagnostics.len());
    assert_eq!(
        Problem::ConstantInitializerExpressionNotAllowed.code(),
        diagnostics[0].code
    );
}

#[test]
fn apply_when_bare_name_is_not_constant_then_p4038() {
    let source = "
PROGRAM main
VAR
    v : UDINT := 7;
    x : UDINT := v;
END_VAR
END_PROGRAM";
    let diagnostics = apply_expect_diagnostics(parse(source, &opts()), &opts());
    assert_eq!(1, diagnostics.len());
    assert_eq!(
        Problem::InitializerNotConstantExpression.code(),
        diagnostics[0].code
    );
}

/// The problem codes of analyzing `C : constant_type := value;` and
/// `x : var_type := C;` in the TwinCAT dialect, with `MyInt` declared as
/// an alias of `UDINT`.
fn named_constant_problems(constant_type: &str, value: &str, var_type: &str) -> Vec<String> {
    let source = format!(
        "
TYPE MyInt : UDINT; END_TYPE
VAR_GLOBAL CONSTANT
    C : {constant_type} := {value};
END_VAR
PROGRAM main
VAR
    x : {var_type} := C;
END_VAR
END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let (_, context) = crate::stages::analyze(&[&library], &options).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

#[rstest::rstest]
#[case::same_type("UDINT", "7", "UDINT")]
#[case::widening("INT", "-7", "DINT")]
#[case::alias_target("UDINT", "7", "MyInt")]
#[case::alias_constant("MyInt", "7", "UDINT")]
#[case::real_widening("REAL", "1.5", "LREAL")]
#[case::temporal_widening("TIME", "T#1s", "LTIME")]
fn resolve_types_when_constant_type_accepted_then_ok(
    #[case] constant_type: &str,
    #[case] value: &str,
    #[case] var_type: &str,
) {
    assert!(named_constant_problems(constant_type, value, var_type).is_empty());
}

#[rstest::rstest]
#[case::narrowing("UDINT", "7", "INT")]
#[case::signed_to_unsigned("INT", "7", "UDINT")]
#[case::int_to_bool("UDINT", "1", "BOOL")]
#[case::bool_to_int("BOOL", "TRUE", "UDINT")]
#[case::real_narrowing("LREAL", "1.5", "REAL")]
#[case::alias_target_narrowing("UDINT", "7", "UINT")]
fn resolve_types_when_constant_type_not_accepted_then_p4022(
    #[case] constant_type: &str,
    #[case] value: &str,
    #[case] var_type: &str,
) {
    assert_eq!(
        vec![Problem::InitializerTypeMismatch.code()],
        named_constant_problems(constant_type, value, var_type)
    );
}
