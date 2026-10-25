//! Spec conformance tests for variable binding (analyzer-owned
//! requirements): which declaration a variable name refers to, what a
//! `VAR_EXTERNAL` stands for, and the check that it matches its global.
//!
//! Each test is annotated with `#[spec_test(REQ_VB_analyzer_NNN)]`, which
//! adds `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every analyzer-owned requirement has a test.
//!
//! The tests resolve source through the pipeline, so they check the
//! declarations the analyzer records and not ones a test inserted by hand.
//!
//! See `specs/design/variable-binding.md`.

use ironplc_dsl::common::VariableType;
use ironplc_dsl::core::{FileId, Id};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::semantic_context::SemanticContext;
use crate::stages::analyze;
use crate::symbol_environment::{DeclRef, Resolution, ScopeKind, ScopePath, SymbolKind};
use crate::test_helpers::{
    codes, configuration_with_globals as config, fb_inheritance_options,
    parse_and_resolve_types_with_options, program_with_externals as program, NOT_IMPLEMENTED_CODE,
};

/// The scope the chain of declaration names `path` opens.
fn scope(path: &[&str]) -> ScopeKind {
    ScopeKind::Named(ScopePath::new(
        path.iter().map(|name| Id::from(name)).collect(),
    ))
}

fn decl(scope: &ScopeKind, name: &str) -> DeclRef {
    DeclRef {
        scope: scope.clone(),
        name: Id::from(name),
    }
}

/// The context resolution builds for `program` under `options`.
fn resolved(program: &str, options: &CompilerOptions) -> SemanticContext {
    parse_and_resolve_types_with_options(program, options).1
}

/// What `name` refers to as a variable from the scope `path` opens.
fn resolve<'a>(context: &'a SemanticContext, name: &str, path: &[&str]) -> Resolution<'a> {
    context
        .symbols()
        .resolve_variable(&Id::from(name), &scope(path))
}

/// The problem codes analysis reports for `program` under `options`.
fn analyze_codes(program: &str, options: &CompilerOptions) -> Vec<String> {
    let library = parse_program(program, &FileId::default(), options).unwrap();
    let (_, context) = analyze(&[&library], options).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect()
}

fn top_level_globals() -> CompilerOptions {
    CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    }
}

fn system_uptime() -> CompilerOptions {
    CompilerOptions {
        allow_system_uptime_global: true,
        ..CompilerOptions::default()
    }
}

// ---------------------------------------------------------------------------
// Naming a declaration
// ---------------------------------------------------------------------------

/// REQ-VB-analyzer-001: The `DeclRef` holds the declaring scope and the name
/// as the declaration spells it.
#[spec_test(REQ_VB_analyzer_001)]
fn analyzer_spec_req_vb_001_decl_ref_is_declaring_scope_and_declared_spelling() {
    let context = resolved(
        "
PROGRAM main
  VAR
    MaxSpeed : INT;
  END_VAR
  MAXSPEED := 1;
END_PROGRAM",
        &CompilerOptions::default(),
    );

    let resolution = resolve(&context, "maxspeed", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. }
            if found.scope == scope(&["main"]) && found.name.original() == "MaxSpeed"
    ));
}

// ---------------------------------------------------------------------------
// Lookup order
// ---------------------------------------------------------------------------

const BLOCK_WITH_METHODS: &str = "
FUNCTION_BLOCK FB_Motor
  VAR
    speed : INT;
  END_VAR
  METHOD Hides
    VAR
      speed : INT;
    END_VAR
    speed := 1;
  END_METHOD
  METHOD Uses
    speed := 2;
  END_METHOD
END_FUNCTION_BLOCK";

/// REQ-VB-analyzer-010: The nearest declaration wins: a method's variable
/// before its block's field, and the field from a method that declares none.
#[rstest]
#[case::method_variable_before_field("Hides", &["FB_Motor", "Hides"])]
#[case::field_from_method("Uses", &["FB_Motor"])]
#[spec_test(REQ_VB_analyzer_010)]
fn analyzer_spec_req_vb_010_nearest_enclosing_declaration(
    #[case] method: &str,
    #[case] declared_in: &[&str],
) {
    let context = resolved(BLOCK_WITH_METHODS, &fb_inheritance_options());

    let resolution = resolve(&context, "speed", &["FB_Motor", method]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&scope(declared_in), "speed")
    ));
}

/// REQ-VB-analyzer-011: An inherited field resolves to the block that
/// declares it.
#[spec_test(REQ_VB_analyzer_011)]
fn analyzer_spec_req_vb_011_inherited_field_names_the_base_block() {
    let context = resolved(
        "
FUNCTION_BLOCK Base
  VAR
    speed : INT;
  END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
  METHOD Uses
    speed := 2;
  END_METHOD
END_FUNCTION_BLOCK",
        &fb_inheritance_options(),
    );

    let resolution = resolve(&context, "speed", &["Derived", "Uses"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&scope(&["Base"]), "speed")
    ));
}

/// REQ-VB-analyzer-012: A unit's own variable is found before a global of
/// the same name.
#[spec_test(REQ_VB_analyzer_012)]
fn analyzer_spec_req_vb_012_local_before_global() {
    let context = resolved(
        &format!(
            "
FUNCTION Twice : DINT
  VAR
    x : DINT;
  END_VAR
  x := 2;
  Twice := x;
END_FUNCTION
{}{}",
            config("x : INT;"),
            program("x : INT;")
        ),
        &CompilerOptions::default(),
    );

    let resolution = resolve(&context, "x", &["Twice"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&scope(&["Twice"]), "x")
                && info.variable_type == Some(VariableType::Var)
    ));
}

/// REQ-VB-analyzer-013: The own name of a function, or of a method with a
/// return type, is its result variable.
#[rstest]
#[case::function(&["Twice"], "Twice")]
#[case::method(&["FB_Motor", "GetSpeed"], "GetSpeed")]
#[spec_test(REQ_VB_analyzer_013)]
fn analyzer_spec_req_vb_013_own_name_is_result_variable(#[case] path: &[&str], #[case] name: &str) {
    let context = resolved(
        "
FUNCTION Twice : DINT
  VAR_INPUT
    n : DINT;
  END_VAR
  Twice := n * 2;
END_FUNCTION

FUNCTION_BLOCK FB_Motor
  VAR
    speed : INT;
  END_VAR
  METHOD GetSpeed : INT
    GetSpeed := speed;
  END_METHOD
END_FUNCTION_BLOCK",
        &fb_inheritance_options(),
    );

    let resolution = resolve(&context, name, path);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&scope(path), name) && info.kind == SymbolKind::ResultVariable
    ));
}

/// REQ-VB-analyzer-014: A name declared as something other than a variable
/// is `NotAVariable`; a name nothing declares is `Undeclared`.
#[spec_test(REQ_VB_analyzer_014)]
fn analyzer_spec_req_vb_014_not_a_variable_and_undeclared() {
    let context = resolved(
        "
TYPE
  Point : STRUCT a : INT; END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    p : Point;
  END_VAR
END_PROGRAM",
        &CompilerOptions::default(),
    );

    assert!(matches!(
        resolve(&context, "Point", &["main"]),
        Resolution::NotAVariable
    ));
    assert!(matches!(
        resolve(&context, "main", &["main"]),
        Resolution::NotAVariable
    ));
    assert!(matches!(
        resolve(&context, "missing", &["main"]),
        Resolution::Undeclared
    ));
}

// ---------------------------------------------------------------------------
// Global visibility
// ---------------------------------------------------------------------------

const PROGRAM_USING_GLOBAL: &str = "
PROGRAM main
  VAR
    y : INT;
  END_VAR
  y := g;
END_PROGRAM
";

/// REQ-VB-analyzer-020: A `VAR_GLOBAL` the body names without a
/// `VAR_EXTERNAL` is `GlobalNotVisible`, and reported as P4007.
#[spec_test(REQ_VB_analyzer_020)]
fn analyzer_spec_req_vb_020_global_without_external_is_not_visible() {
    let source = format!("{}{PROGRAM_USING_GLOBAL}", config("g : INT;"));
    let options = CompilerOptions::default();
    let context = resolved(&source, &options);

    let resolution = resolve(&context, "g", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::GlobalNotVisible { decl: found, .. } if found == decl(&ScopeKind::Global, "g")
    ));
    assert_eq!(
        analyze_codes(&source, &options),
        codes(&[Problem::VariableUndefined])
    );
}

/// REQ-VB-analyzer-021: With `allow_top_level_var_global`, a body uses a
/// global directly.
#[spec_test(REQ_VB_analyzer_021)]
fn analyzer_spec_req_vb_021_top_level_globals_are_visible() {
    let source = format!("VAR_GLOBAL g : INT; END_VAR{PROGRAM_USING_GLOBAL}");
    let options = top_level_globals();
    let context = resolved(&source, &options);

    let resolution = resolve(&context, "g", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&ScopeKind::Global, "g")
    ));
    assert_eq!(analyze_codes(&source, &options), codes(&[]));
}

/// REQ-VB-analyzer-022: A compiler-provided global is visible without a
/// `VAR_EXTERNAL`, on a dialect that does not let a body use globals
/// directly.
#[spec_test(REQ_VB_analyzer_022)]
fn analyzer_spec_req_vb_022_compiler_provided_global_is_visible() {
    let source = "
PROGRAM main
  VAR
    t : TIME;
  END_VAR
  t := __SYSTEM_UP_TIME;
END_PROGRAM";
    let options = system_uptime();
    let context = resolved(source, &options);

    let resolution = resolve(&context, "__SYSTEM_UP_TIME", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&ScopeKind::Global, "__SYSTEM_UP_TIME") && info.compiler_provided
    ));
    assert_eq!(analyze_codes(source, &options), codes(&[]));
}

// ---------------------------------------------------------------------------
// A VAR_EXTERNAL is its global
// ---------------------------------------------------------------------------

/// REQ-VB-analyzer-030: A name that reaches a `VAR_EXTERNAL` resolves to the
/// global, spelled as the global's declaration spells it.
#[spec_test(REQ_VB_analyzer_030)]
fn analyzer_spec_req_vb_030_external_resolves_to_its_global() {
    let context = resolved(
        &format!(
            "{}{}",
            config("MaxSpeed : INT;"),
            program("MAXSPEED : INT;")
        ),
        &CompilerOptions::default(),
    );

    let resolution = resolve(&context, "maxspeed", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. }
            if found.scope == ScopeKind::Global && found.name.original() == "MaxSpeed"
    ));
}

/// REQ-VB-analyzer-031: Two units that name one global through an external
/// resolve to the same `DeclRef`.
#[spec_test(REQ_VB_analyzer_031)]
fn analyzer_spec_req_vb_031_externals_of_one_global_are_one_declaration() {
    let context = resolved(
        &format!(
            "
FUNCTION_BLOCK Counter
  VAR_EXTERNAL
    g : INT;
  END_VAR
  g := g + 1;
END_FUNCTION_BLOCK
{}{}",
            config("g : INT;"),
            program("g : INT;")
        ),
        &CompilerOptions::default(),
    );

    let from_program = resolve(&context, "g", &["main"]);
    let from_block = resolve(&context, "g", &["Counter"]);

    assert!(matches!(
        (from_program, from_block),
        (
            Resolution::Variable { decl: program, .. },
            Resolution::Variable { decl: block, .. }
        ) if program == block && program == decl(&ScopeKind::Global, "g")
    ));
}

/// REQ-VB-analyzer-032: Through a `VAR_EXTERNAL`, `info` is the external
/// declaration: here it is `CONSTANT` and the global is not.
#[spec_test(REQ_VB_analyzer_032)]
fn analyzer_spec_req_vb_032_info_describes_the_external_declaration() {
    let context = resolved(
        &format!(
            "{}
PROGRAM main
  VAR_EXTERNAL CONSTANT
    g : INT;
  END_VAR
END_PROGRAM",
            config("g : INT;")
        ),
        &CompilerOptions::default(),
    );

    let resolution = resolve(&context, "g", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found.scope == ScopeKind::Global
                && info.scope == scope(&["main"])
                && info.is_external
                && info.is_constant()
    ));
    assert!(context
        .symbols()
        .global_variable(&Id::from("g"))
        .is_some_and(|(_, global)| !global.is_constant()));
}

/// REQ-VB-analyzer-033: A `VAR_EXTERNAL` may name a compiler-provided
/// global.
#[spec_test(REQ_VB_analyzer_033)]
fn analyzer_spec_req_vb_033_external_names_compiler_provided_global() {
    let source = program("__SYSTEM_UP_TIME : TIME;");
    let options = system_uptime();
    let context = resolved(&source, &options);

    let resolution = resolve(&context, "__SYSTEM_UP_TIME", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&ScopeKind::Global, "__SYSTEM_UP_TIME") && info.is_external
    ));
    assert_eq!(analyze_codes(&source, &options), codes(&[]));
}

/// REQ-VB-analyzer-034: A `VAR_EXTERNAL` that names no global variable
/// resolves to its own declaration.
#[spec_test(REQ_VB_analyzer_034)]
fn analyzer_spec_req_vb_034_external_without_global_is_its_own_declaration() {
    let context = resolved(
        &format!("{}{}", config("g : INT;"), program("missing : INT;")),
        &CompilerOptions::default(),
    );

    let resolution = resolve(&context, "missing", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&scope(&["main"]), "missing")
    ));
}

// ---------------------------------------------------------------------------
// Checking a VAR_EXTERNAL
// ---------------------------------------------------------------------------

const TYPES: &str = "
TYPE
  Point : STRUCT a : INT; b : INT; END_STRUCT;
  Vector : STRUCT a : INT; b : INT; END_STRUCT;
  Speed : INT;
END_TYPE
";

const OTHER_UNIT: &str = "
FUNCTION_BLOCK Other
  VAR
    local : INT;
  END_VAR
END_FUNCTION_BLOCK
";

/// REQ-VB-analyzer-040: A `VAR_EXTERNAL` whose name no global variable has
/// is P4082.
#[rstest]
#[case::nothing_of_that_name("missing : INT;")]
#[case::a_type("Point : INT;")]
#[case::another_units_variable("local : INT;")]
#[spec_test(REQ_VB_analyzer_040)]
fn analyzer_spec_req_vb_040_external_without_global_is_p4082(#[case] externals: &str) {
    let source = format!(
        "{TYPES}{OTHER_UNIT}{}{}",
        config("g : INT;"),
        program(externals)
    );

    let reported = analyze_codes(&source, &CompilerOptions::default());

    assert_eq!(reported, codes(&[Problem::ExternalMatchesNoGlobal]));
}

/// REQ-VB-analyzer-041: A `VAR_EXTERNAL` declared with another type than its
/// global's is P4082, and one declared with the global's type is not.
#[rstest]
#[case::same_type("g : INT;", &[])]
#[case::another_type("g : DINT;", &[Problem::ExternalMatchesNoGlobal])]
#[spec_test(REQ_VB_analyzer_041)]
fn analyzer_spec_req_vb_041_external_of_another_type_is_p4082(
    #[case] externals: &str,
    #[case] expected: &[Problem],
) {
    let source = format!("{}{}", config("g : INT;"), program(externals));

    let reported = analyze_codes(&source, &CompilerOptions::default());

    assert_eq!(reported, codes(expected));
}

/// REQ-VB-analyzer-042: An alias and the type it names are different types,
/// and so are two structure types with the same fields.
#[rstest]
#[case::alias_and_its_base("g : Speed;", "g : INT;")]
#[case::base_and_its_alias("g : INT;", "g : Speed;")]
#[case::structures_of_one_shape("g : Point;", "g : Vector;")]
#[spec_test(REQ_VB_analyzer_042)]
fn analyzer_spec_req_vb_042_types_are_compared_by_identity(
    #[case] globals: &str,
    #[case] externals: &str,
) {
    let source = format!("{TYPES}{}{}", config(globals), program(externals));

    let reported = analyze_codes(&source, &CompilerOptions::default());

    assert_eq!(reported, codes(&[Problem::ExternalMatchesNoGlobal]));
}

/// REQ-VB-analyzer-043: Two arrays spelled in place are the same type when
/// their bounds and element type are the same.
#[rstest]
#[case::same("ARRAY[1..3] OF INT", &[])]
#[case::other_bounds("ARRAY[0..2] OF INT", &[Problem::ExternalMatchesNoGlobal])]
#[case::other_dimensions("ARRAY[1..1, 1..3] OF INT", &[Problem::ExternalMatchesNoGlobal])]
#[case::other_element_type("ARRAY[1..3] OF DINT", &[Problem::ExternalMatchesNoGlobal])]
#[spec_test(REQ_VB_analyzer_043)]
fn analyzer_spec_req_vb_043_arrays_in_place_are_compared_by_shape(
    #[case] external_type: &str,
    #[case] expected: &[Problem],
) {
    let source = format!(
        "{}{}",
        config("g : ARRAY[1..3] OF INT;"),
        program(&format!("g : {external_type};"))
    );

    let reported = analyze_codes(&source, &CompilerOptions::default());

    assert_eq!(reported, codes(expected));
}

/// REQ-VB-analyzer-044: P4082 is reported on every dialect.
#[rstest]
#[case::no_global("missing : INT;")]
#[case::another_type("g : DINT;")]
#[spec_test(REQ_VB_analyzer_044)]
fn analyzer_spec_req_vb_044_p4082_on_every_dialect(#[case] externals: &str) {
    let source = format!("{}{}", config("g : INT;"), program(externals));

    let reported: Vec<Vec<String>> = Dialect::ALL
        .iter()
        .map(|dialect| analyze_codes(&source, &CompilerOptions::from_dialect(*dialect)))
        .collect();

    assert_eq!(
        reported,
        vec![codes(&[Problem::ExternalMatchesNoGlobal]); Dialect::ALL.len()]
    );
}

// ---------------------------------------------------------------------------
// Structure fields
// ---------------------------------------------------------------------------

const GLOBAL_X: &str = "
VAR_GLOBAL
  x : INT := 7;
END_VAR
";

const STRUCTURE_WITH_FIELD_X: &str = "
TYPE
  Sample : STRUCT x : INT; END_STRUCT;
END_TYPE
";

/// REQ-VB-analyzer-050: A structure field is declared in its structure
/// type's scope, so it and a global variable of the same name are both
/// declared, in either order.
#[rstest]
#[case::global_first(GLOBAL_X, STRUCTURE_WITH_FIELD_X)]
#[case::structure_first(STRUCTURE_WITH_FIELD_X, GLOBAL_X)]
#[spec_test(REQ_VB_analyzer_050)]
fn analyzer_spec_req_vb_050_field_and_global_of_one_name_are_both_declared(
    #[case] first: &str,
    #[case] second: &str,
) {
    let source = format!(
        "{first}{second}
PROGRAM main
  VAR
    s : Sample;
    y : INT;
  END_VAR
  y := x;
  s.x := y;
END_PROGRAM"
    );
    let options = top_level_globals();
    let context = resolved(&source, &options);

    let resolution = resolve(&context, "x", &["main"]);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&ScopeKind::Global, "x")
                && info.variable_type == Some(VariableType::Global)
    ));
    assert!(context
        .symbols()
        .find(&Id::from("x"), &scope(&["Sample"]))
        .is_some_and(|field| field.kind == SymbolKind::StructureElement));
    assert_eq!(analyze_codes(&source, &options), codes(&[]));
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

const BLOCK_WITH_PROPERTY: &str = "
FUNCTION_BLOCK FB_Axis
  VAR
    _pos : INT;
  END_VAR
  METHOD Reset
    Position := 0;
  END_METHOD
  PROPERTY Position : INT
  GET
    VAR
      tmp : INT;
    END_VAR
    tmp := _pos;
    Position := tmp;
  END_GET
  SET
    VAR
      tmp : INT;
    END_VAR
    tmp := Position;
    _pos := tmp;
  END_SET
  END_PROPERTY
END_FUNCTION_BLOCK";

/// REQ-VB-analyzer-060: A property is declared in its block's scope, and a
/// name that reaches it is not a variable; its use is not implemented.
#[spec_test(REQ_VB_analyzer_060)]
fn analyzer_spec_req_vb_060_property_is_declared_in_block_scope_and_not_a_variable() {
    let options = fb_inheritance_options();
    let context = resolved(BLOCK_WITH_PROPERTY, &options);

    assert!(context
        .symbols()
        .find(&Id::from("Position"), &scope(&["FB_Axis"]))
        .is_some_and(|property| property.kind == SymbolKind::Property));
    assert!(matches!(
        resolve(&context, "Position", &["FB_Axis", "Reset"]),
        Resolution::NotAVariable
    ));
    assert_eq!(
        analyze_codes(BLOCK_WITH_PROPERTY, &options),
        [NOT_IMPLEMENTED_CODE]
    );
}

/// REQ-VB-analyzer-061: A property hides a global of the same name, and an
/// inherited property is found from the derived block.
#[spec_test(REQ_VB_analyzer_061)]
fn analyzer_spec_req_vb_061_property_hides_global_and_is_inherited() {
    let source = "
VAR_GLOBAL
  Speed : REAL;
END_VAR

FUNCTION_BLOCK FB_Base
  VAR
    _speed : REAL;
  END_VAR
  PROPERTY Speed : REAL
  GET
    Speed := _speed;
  END_GET
  END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
  METHOD Stop
    Speed := 0.0;
  END_METHOD
END_FUNCTION_BLOCK";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    };
    let context = resolved(source, &options);

    assert!(matches!(
        resolve(&context, "Speed", &["FB_Base"]),
        Resolution::NotAVariable
    ));
    assert!(matches!(
        resolve(&context, "Speed", &["FB_Motor", "Stop"]),
        Resolution::NotAVariable
    ));
    assert!(matches!(
        resolve(&context, "Speed", &["FB_Base", "Speed.GET"]),
        Resolution::Variable { .. }
    ));
}

/// REQ-VB-analyzer-062: A property and a variable of one name in one block
/// are a repeated declaration.
#[spec_test(REQ_VB_analyzer_062)]
fn analyzer_spec_req_vb_062_property_and_variable_of_one_name_are_repeated() {
    let source = "
FUNCTION_BLOCK FB_Axis
  VAR
    Position : INT;
  END_VAR
  PROPERTY Position : INT
  GET
    Position := 1;
  END_GET
  END_PROPERTY
END_FUNCTION_BLOCK";

    assert!(analyze_codes(source, &fb_inheritance_options())
        .contains(&Problem::SymbolDeclDuplicated.code().to_string()));
}

/// REQ-VB-analyzer-063: Each accessor has a scope of its own, so a variable
/// each declares under one name is two declarations, and the property's
/// name is the accessor's own variable.
#[spec_test(REQ_VB_analyzer_063)]
fn analyzer_spec_req_vb_063_each_accessor_has_its_own_scope() {
    let options = fb_inheritance_options();
    let context = resolved(BLOCK_WITH_PROPERTY, &options);
    let get = ["FB_Axis", "Position.GET"];
    let set = ["FB_Axis", "Position.SET"];

    assert!(matches!(
        resolve(&context, "tmp", &get),
        Resolution::Variable { decl: found, .. } if found == decl(&scope(&get), "tmp")
    ));
    assert!(matches!(
        resolve(&context, "tmp", &set),
        Resolution::Variable { decl: found, .. } if found == decl(&scope(&set), "tmp")
    ));
    assert!(matches!(
        resolve(&context, "Position", &get),
        Resolution::Variable { decl: found, info }
            if found == decl(&scope(&get), "Position") && info.kind == SymbolKind::ResultVariable
    ));
    assert!(matches!(
        resolve(&context, "Position", &set),
        Resolution::Variable { decl: found, info }
            if found == decl(&scope(&set), "Position") && info.kind == SymbolKind::Parameter
    ));
    assert!(!analyze_codes(BLOCK_WITH_PROPERTY, &options)
        .contains(&Problem::SymbolDeclDuplicated.code().to_string()));
}

/// REQ-VB-analyzer-064: A variable a derived block declares with the name of
/// a property a base block declares is P4044.
#[spec_test(REQ_VB_analyzer_064)]
fn analyzer_spec_req_vb_064_variable_named_as_inherited_property_is_p4044() {
    let source = "
FUNCTION_BLOCK FB_Base
  VAR
    _pos : INT;
  END_VAR
  PROPERTY Position : INT
  GET
    Position := _pos;
  END_GET
  END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Middle EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Middle
  VAR
    Position : INT;
  END_VAR
END_FUNCTION_BLOCK";

    assert_eq!(
        analyze_codes(source, &fb_inheritance_options()),
        codes(&[Problem::ExtendsFieldNameDuplicated])
    );
}
