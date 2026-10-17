//! Tests of which declaration a name refers to as a variable:
//! [`SymbolEnvironment::resolve_variable`] and the list of names it would
//! accept, [`SymbolEnvironment::usable_variables`].

use rstest::rstest;

use super::tests::function_block;
use super::*;

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

/// Declares the variable `name` in `scope`, in a `VAR*` block of the kind
/// `variable_type` names.
fn declare(
    env: &mut SymbolEnvironment,
    scope: &ScopeKind,
    name: &str,
    variable_type: VariableType,
) {
    env.insert_variable(
        &Id::from(name),
        scope,
        variable_type,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();
}

#[test]
fn resolve_variable_when_local_and_global_share_name_then_local() {
    let mut env = SymbolEnvironment::new();
    let function = scope(&["F"]);
    declare(&mut env, &ScopeKind::Global, "x", VariableType::Global);
    declare(&mut env, &function, "x", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("x"), &function);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&function, "x") && info.variable_type == Some(VariableType::Var)
    ));
}

#[test]
fn resolve_variable_when_method_local_and_field_share_name_then_method_local() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "FB", None);
    let method = scope(&["FB", "M"]);
    declare(&mut env, &scope(&["FB"]), "speed", VariableType::Var);
    declare(&mut env, &method, "speed", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("speed"), &method);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&method, "speed")
    ));
}

#[test]
fn resolve_variable_when_field_used_from_method_then_names_the_block() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "FB", None);
    let block = scope(&["FB"]);
    declare(&mut env, &block, "speed", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("speed"), &scope(&["FB", "M"]));

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&block, "speed")
    ));
}

#[test]
fn resolve_variable_when_field_inherited_then_names_the_base_block() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Derived", Some("Base"));
    let base = scope(&["Base"]);
    declare(&mut env, &base, "speed", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("speed"), &scope(&["Derived", "M"]));

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&base, "speed")
    ));
}

#[test]
fn resolve_variable_when_function_name_then_result_variable() {
    let mut env = SymbolEnvironment::new();
    let function = scope(&["F"]);
    let name = Id::from("F");
    let info = SymbolInfo::new(SymbolKind::ResultVariable, function.clone(), name.span());
    env.insert_info(&name, info).unwrap();

    let resolution = env.resolve_variable(&name, &function);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&function, "F") && info.kind == SymbolKind::ResultVariable
    ));
}

#[test]
fn resolve_variable_when_nothing_declares_name_then_undeclared() {
    let mut env = SymbolEnvironment::new();
    let function = scope(&["F"]);
    declare(&mut env, &function, "x", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("y"), &function);

    assert!(matches!(resolution, Resolution::Undeclared));
}

#[test]
fn resolve_variable_when_declared_in_another_scope_then_undeclared() {
    let mut env = SymbolEnvironment::new();
    declare(&mut env, &scope(&["F"]), "x", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("x"), &scope(&["G"]));

    assert!(matches!(resolution, Resolution::Undeclared));
}

#[rstest]
#[case::data_type(SymbolKind::Type)]
#[case::program(SymbolKind::Program)]
#[case::function_block(SymbolKind::FunctionBlock)]
#[case::configuration(SymbolKind::Configuration)]
#[case::structure_element(SymbolKind::StructureElement)]
fn resolve_variable_when_name_declared_as_other_kind_then_not_a_variable(#[case] kind: SymbolKind) {
    let mut env = SymbolEnvironment::new();
    env.insert(&Id::from("thing"), kind, &ScopeKind::Global)
        .unwrap();

    let resolution = env.resolve_variable(&Id::from("thing"), &scope(&["F"]));

    assert!(matches!(resolution, Resolution::NotAVariable));
}

#[test]
fn resolve_variable_when_nearer_declaration_not_a_variable_then_hides_farther_variable() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "FB", None);
    let method = scope(&["FB", "M"]);
    declare(&mut env, &scope(&["FB"]), "a", VariableType::Var);
    env.insert(&Id::from("a"), SymbolKind::StructureElement, &method)
        .unwrap();

    let resolution = env.resolve_variable(&Id::from("a"), &method);

    assert!(matches!(resolution, Resolution::NotAVariable));
}

#[test]
fn resolve_variable_when_global_without_external_then_global_not_visible() {
    let mut env = SymbolEnvironment::new();
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);

    let resolution = env.resolve_variable(&Id::from("g"), &scope(&["main"]));

    assert!(matches!(
        resolution,
        Resolution::GlobalNotVisible { decl: found, info }
            if found == decl(&ScopeKind::Global, "g")
                && info.variable_type == Some(VariableType::Global)
    ));
}

#[test]
fn resolve_variable_when_global_compiler_provided_then_variable() {
    let mut env = SymbolEnvironment::new();
    env.insert_compiler_provided(
        &Id::from("uptime"),
        SymbolKind::Variable,
        &ScopeKind::Global,
        None,
    )
    .unwrap();

    let resolution = env.resolve_variable(&Id::from("uptime"), &scope(&["main"]));

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, info }
            if found == decl(&ScopeKind::Global, "uptime") && info.compiler_provided
    ));
}

#[test]
fn resolve_variable_when_bare_globals_then_global_is_variable() {
    let mut env = SymbolEnvironment::new().with_bare_globals(true);
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);

    let resolution = env.resolve_variable(&Id::from("g"), &scope(&["main"]));

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&ScopeKind::Global, "g")
    ));
}

#[test]
fn resolve_variable_when_external_then_names_the_global() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);
    declare(&mut env, &program, "g", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("g"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&ScopeKind::Global, "g")
    ));
}

#[test]
fn resolve_variable_when_external_then_info_is_the_external_declaration() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);
    env.insert_variable(
        &Id::from("g"),
        &program,
        VariableType::External,
        DeclarationQualifier::Constant,
        None,
        None,
    )
    .unwrap();

    let resolution = env.resolve_variable(&Id::from("g"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { info, .. }
            if info.is_external && info.is_constant() && info.scope == program
    ));
}

#[test]
fn resolve_variable_when_two_units_declare_one_external_then_same_declaration() {
    let mut env = SymbolEnvironment::new();
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);
    declare(&mut env, &scope(&["main"]), "g", VariableType::External);
    declare(&mut env, &scope(&["FB"]), "g", VariableType::External);

    let from_program = env.resolve_variable(&Id::from("g"), &scope(&["main"]));
    let from_block = env.resolve_variable(&Id::from("g"), &scope(&["FB"]));

    assert!(matches!(
        (from_program, from_block),
        (
            Resolution::Variable { decl: program, .. },
            Resolution::Variable { decl: block, .. }
        ) if program == block
    ));
}

#[test]
fn resolve_variable_when_external_names_global_in_other_case_then_global_spelling() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    declare(
        &mut env,
        &ScopeKind::Global,
        "MaxSpeed",
        VariableType::Global,
    );
    declare(&mut env, &program, "MAXSPEED", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("maxspeed"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found.name.original() == "MaxSpeed"
    ));
}

#[test]
fn resolve_variable_when_external_names_compiler_provided_global_then_names_the_global() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    env.insert_compiler_provided(
        &Id::from("uptime"),
        SymbolKind::Variable,
        &ScopeKind::Global,
        None,
    )
    .unwrap();
    declare(&mut env, &program, "uptime", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("uptime"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&ScopeKind::Global, "uptime")
    ));
}

#[test]
fn resolve_variable_when_external_names_no_global_then_its_own_declaration() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    declare(&mut env, &program, "g", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("g"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&program, "g")
    ));
}

#[test]
fn resolve_variable_when_external_names_a_type_then_its_own_declaration() {
    let mut env = SymbolEnvironment::new();
    let program = scope(&["main"]);
    env.insert(&Id::from("g"), SymbolKind::Type, &ScopeKind::Global)
        .unwrap();
    declare(&mut env, &program, "g", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("g"), &program);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&program, "g")
    ));
}

#[test]
fn resolve_variable_when_external_inherited_then_names_the_global() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Derived", Some("Base"));
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);
    declare(&mut env, &scope(&["Base"]), "g", VariableType::External);

    let resolution = env.resolve_variable(&Id::from("g"), &scope(&["Derived", "M"]));

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found == decl(&ScopeKind::Global, "g")
    ));
}

#[test]
fn global_variable_when_var_global_then_found_with_declaration_spelling() {
    let mut env = SymbolEnvironment::new();
    declare(
        &mut env,
        &ScopeKind::Global,
        "MaxSpeed",
        VariableType::Global,
    );

    let found = env.global_variable(&Id::from("MAXSPEED"));

    assert!(found.is_some_and(|(name, info)| {
        name.original() == "MaxSpeed" && info.variable_type == Some(VariableType::Global)
    }));
}

#[rstest]
#[case::data_type(SymbolKind::Type)]
#[case::program(SymbolKind::Program)]
#[case::function_block(SymbolKind::FunctionBlock)]
fn global_variable_when_name_declared_as_other_kind_then_none(#[case] kind: SymbolKind) {
    let mut env = SymbolEnvironment::new();
    env.insert(&Id::from("thing"), kind, &ScopeKind::Global)
        .unwrap();

    assert!(env.global_variable(&Id::from("thing")).is_none());
}

#[test]
fn global_variable_when_declared_in_a_unit_only_then_none() {
    let mut env = SymbolEnvironment::new();
    declare(&mut env, &scope(&["main"]), "x", VariableType::Var);

    assert!(env.global_variable(&Id::from("x")).is_none());
}

#[test]
fn resolve_variable_when_reference_spelled_in_other_case_then_declaration_spelling() {
    let mut env = SymbolEnvironment::new();
    let function = scope(&["F"]);
    declare(&mut env, &function, "Speed", VariableType::Var);

    let resolution = env.resolve_variable(&Id::from("SPEED"), &function);

    assert!(matches!(
        resolution,
        Resolution::Variable { decl: found, .. } if found.name.original() == "Speed"
    ));
}

/// The names [`SymbolEnvironment::usable_variables`] lists from `from`.
fn usable_names(env: &SymbolEnvironment, from: &ScopeKind) -> Vec<String> {
    env.usable_variables(from)
        .iter()
        .map(|(name, _)| name.to_string())
        .collect()
}

/// An environment in which `main` declares `x` and an external `e`, and
/// the globals are `e`, `g` and a compiler-provided `uptime`.
fn program_with_globals(bare_globals: bool) -> SymbolEnvironment {
    let mut env = SymbolEnvironment::new().with_bare_globals(bare_globals);
    let program = scope(&["main"]);
    declare(&mut env, &program, "x", VariableType::Var);
    declare(&mut env, &program, "e", VariableType::External);
    declare(&mut env, &ScopeKind::Global, "e", VariableType::Global);
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);
    env.insert_compiler_provided(
        &Id::from("uptime"),
        SymbolKind::Variable,
        &ScopeKind::Global,
        None,
    )
    .unwrap();
    env
}

#[test]
fn usable_variables_when_global_without_external_then_not_listed() {
    let env = program_with_globals(false);

    let names = usable_names(&env, &scope(&["main"]));

    assert_eq!(names, vec!["x", "e", "uptime"]);
}

#[test]
fn usable_variables_when_bare_globals_then_global_listed() {
    let env = program_with_globals(true);

    let names = usable_names(&env, &scope(&["main"]));

    assert_eq!(names, vec!["x", "e", "g", "uptime"]);
}

#[test]
fn declared_when_variable_then_its_symbol() {
    let mut env = SymbolEnvironment::new();
    let function = scope(&["F"]);
    declare(&mut env, &function, "x", VariableType::Var);

    let declared = env.resolve_variable(&Id::from("x"), &function).declared();

    assert!(declared.is_some_and(|info| info.variable_type == Some(VariableType::Var)));
}

#[test]
fn declared_when_global_not_visible_then_its_symbol() {
    let mut env = SymbolEnvironment::new();
    declare(&mut env, &ScopeKind::Global, "g", VariableType::Global);

    let declared = env
        .resolve_variable(&Id::from("g"), &scope(&["main"]))
        .declared();

    assert!(declared.is_some_and(|info| info.variable_type == Some(VariableType::Global)));
}

#[test]
fn declared_when_not_a_variable_then_none() {
    let mut env = SymbolEnvironment::new();
    env.insert(&Id::from("T"), SymbolKind::Type, &ScopeKind::Global)
        .unwrap();

    let declared = env
        .resolve_variable(&Id::from("T"), &scope(&["F"]))
        .declared();

    assert!(declared.is_none());
}

#[test]
fn declared_when_undeclared_then_none() {
    let env = SymbolEnvironment::new();

    let declared = env
        .resolve_variable(&Id::from("x"), &scope(&["F"]))
        .declared();

    assert!(declared.is_none());
}
