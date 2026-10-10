use super::*;
use ironplc_dsl::common::DeclarationQualifier;
use ironplc_dsl::core::Id;

#[test]
fn symbol_environment_basic_operations_when_inserting_and_finding_symbols_then_works_correctly() {
    let mut env = SymbolEnvironment::new();

    // Test inserting global symbols
    let id1 = Id::from("GLOBAL_VAR");
    let id2 = Id::from("FUNCTION_NAME");

    env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();
    env.insert(&id2, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();

    // Test finding symbols
    let symbol1 = env.find(&id1, &ScopeKind::Global).unwrap();
    assert_eq!(symbol1.kind, SymbolKind::Variable);

    let symbol2 = env.find(&id2, &ScopeKind::Global).unwrap();
    assert_eq!(symbol2.kind, SymbolKind::Program);

    // Test scoped symbols
    let scope = ScopeKind::Named(Id::from("FUNCTION_BLOCK").into());
    let id3 = Id::from("LOCAL_VAR");

    env.insert(&id3, SymbolKind::Variable, &scope).unwrap();

    let symbol3 = env.find(&id3, &scope).unwrap();
    assert_eq!(symbol3.kind, SymbolKind::Variable);

    // Test scope hierarchy (local scope should find global symbols)
    let symbol1_in_scope = env.find(&id1, &scope).unwrap();
    assert_eq!(symbol1_in_scope.kind, SymbolKind::Variable);
}

#[test]
fn symbol_environment_scope_management_when_managing_scopes_then_symbols_are_in_correct_scopes() {
    let mut env = SymbolEnvironment::new();

    let global_id = Id::from("GLOBAL");
    let function_id = Id::from("FUNCTION");
    let local_id = Id::from("LOCAL");

    // Insert global symbol
    env.insert(&global_id, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();

    // Insert function symbol
    env.insert(&function_id, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();

    // Insert local symbol in function scope
    let function_scope = ScopeKind::Named(function_id.clone().into());
    env.insert(&local_id, SymbolKind::Variable, &function_scope)
        .unwrap();

    // Verify symbols are in correct scopes
    assert!(env.find(&global_id, &ScopeKind::Global).is_some());
    assert!(env.find(&function_id, &ScopeKind::Global).is_some());
    assert!(env.find(&local_id, &function_scope).is_some());

    // Verify local symbol is not visible globally
    assert!(env.find(&local_id, &ScopeKind::Global).is_none());

    // Verify global symbols are visible from local scope
    assert!(env.find(&global_id, &function_scope).is_some());
}

#[test]
fn get_when_checking_symbol_existence_then_returns_correct_results() {
    let mut env = SymbolEnvironment::new();

    let id1 = Id::from("GLOBAL_VAR");
    let id2 = Id::from("LOCAL_VAR");

    // Insert global symbol
    env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    // Test get method (alias for find)
    let symbol1 = env.get(&id1, &ScopeKind::Global).unwrap();
    assert_eq!(symbol1.kind, SymbolKind::Variable);

    let symbol2 = env.get(&id2, &ScopeKind::Global);
    assert!(symbol2.is_none());
}

#[test]
fn default_implementation_when_creating_default_then_creates_empty_environment() {
    let env = SymbolEnvironment::default();

    // Default should create an empty environment
    assert_eq!(env.all_symbols().count(), 0);

    // Should be equivalent to new()
    let env2 = SymbolEnvironment::new();
    assert_eq!(env.all_symbols().count(), env2.all_symbols().count());
}

#[test]
fn debug_implementation_when_debugging_then_formats_correctly() {
    let mut env = SymbolEnvironment::new();

    // Test debug output for empty environment
    let debug_output = format!("{env:?}");
    assert!(debug_output.contains("SymbolEnvironment"));
    assert!(debug_output.contains("global_symbols"));
    assert!(debug_output.contains("scoped_symbols"));

    // Test debug output with symbols
    let id = Id::from("TEST_VAR");
    env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let debug_output = format!("{env:?}");
    assert!(debug_output.contains("SymbolEnvironment"));
    assert!(debug_output.contains("global_symbols"));
    assert!(debug_output.contains("scoped_symbols"));
}

#[test]
fn scope_kind_variants_when_creating_scope_kinds_then_creates_correct_variants() {
    // Test Global scope
    let global_scope = ScopeKind::Global;
    assert_eq!(global_scope, ScopeKind::Global);

    // Test Named scope
    let function_id = Id::from("TEST_FUNCTION");
    let named_scope = ScopeKind::Named(function_id.clone().into());
    assert_eq!(named_scope, ScopeKind::Named(function_id.into()));

    // Test scope comparison
    assert_ne!(global_scope, named_scope);

    // Test scope cloning
    let cloned_scope = named_scope.clone();
    assert_eq!(named_scope, cloned_scope);
}

#[test]
fn edge_cases_and_error_conditions_when_handling_edge_cases_then_handles_correctly() {
    let mut env = SymbolEnvironment::new();

    // A name declared twice in one scope is reported rather than
    // silently overwriting the first declaration.
    let id = Id::from("DUPLICATE_VAR");
    env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();
    assert!(env
        .insert(&id, SymbolKind::Variable, &ScopeKind::Global)
        .is_err());

    // Test finding symbol in wrong scope
    let global_id = Id::from("GLOBAL_ONLY");
    env.insert(&global_id, SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let wrong_scope = ScopeKind::Named(Id::from("WRONG_FUNCTION").into());
    let found = env.find(&global_id, &wrong_scope);
    // Global symbols are accessible from any scope, so this should find the symbol
    assert!(found.is_some());

    // Test scope hierarchy with non-existent scope
    let non_existent_scope = ScopeKind::Named(Id::from("NON_EXISTENT").into());
    let found = env.find(&global_id, &non_existent_scope);
    assert!(found.is_some()); // Should find in global scope
}

#[test]
fn get_enumeration_values_for_type_when_values_of_two_types_then_returns_matching_only() {
    let mut env = SymbolEnvironment::new();
    let enum_type = TypeName::from("COLOR");
    let other_type = TypeName::from("SIZE");
    env.insert_enumeration_value(&Id::from("RED"), &enum_type);
    env.insert_enumeration_value(&Id::from("SMALL"), &other_type);
    env.insert_enumeration_value(&Id::from("GREEN"), &enum_type);
    // A variable is not an enumeration value.
    env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let values = env.get_enumeration_values_for_type(&enum_type);

    assert_eq!(values, vec![&Id::from("RED"), &Id::from("GREEN")]);
}

#[test]
fn get_enumeration_values_for_type_when_two_types_share_value_then_each_keeps_it() {
    let mut env = SymbolEnvironment::new();
    let colors = TypeName::from("Colors");
    let lights = TypeName::from("Lights");
    env.insert_enumeration_value(&Id::from("Red"), &colors);
    env.insert_enumeration_value(&Id::from("Green"), &colors);
    env.insert_enumeration_value(&Id::from("Red"), &lights);

    assert_eq!(
        env.get_enumeration_values_for_type(&colors),
        vec![&Id::from("Red"), &Id::from("Green")]
    );
    assert_eq!(
        env.get_enumeration_values_for_type(&lights),
        vec![&Id::from("Red")]
    );
}

#[test]
fn insert_when_global_shares_enumeration_value_name_then_both_kept() {
    let mut env = SymbolEnvironment::new();
    let colors = TypeName::from("Colors");
    env.insert_enumeration_value(&Id::from("Red"), &colors);
    global(&mut env, "Red", SymbolKind::Variable).unwrap();
    env.insert_enumeration_alias(&TypeName::from("Paint"), &colors);

    let global = env.find(&Id::from("Red"), &ScopeKind::Global).unwrap();
    assert_eq!(global.kind, SymbolKind::Variable);
    assert_eq!(
        env.get_enumeration_values_for_type(&colors),
        vec![&Id::from("Red")]
    );
}

#[test]
fn get_enumeration_values_for_type_when_no_matching_values_then_returns_empty() {
    let mut env = SymbolEnvironment::new();
    env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let values = env.get_enumeration_values_for_type(&TypeName::from("COLOR"));
    assert!(values.is_empty());
}

#[test]
fn symbol_info_span_and_scope_when_creating_symbol_info_then_has_correct_span_and_scope() {
    let span = ironplc_dsl::core::SourceSpan::default();
    let scope = ScopeKind::Named(Id::from("TEST_FUNCTION").into());

    let symbol_info = SymbolInfo::new(SymbolKind::Variable, scope.clone(), span);

    // Test that scope and visibility_scope are set correctly
    assert_eq!(symbol_info.scope, scope);
    assert_eq!(symbol_info.visibility_scope, scope);
    assert_eq!(symbol_info.span, ironplc_dsl::core::SourceSpan::default());
    assert!(!symbol_info.is_external);
    assert!(symbol_info.type_id.is_none());
}

/// A scope path nests, so a symbol declared in an enclosing scope is
/// visible from an inner one -- how a method body sees the fields of
/// the function block it is declared on.
#[test]
fn find_when_symbol_is_in_enclosing_scope_then_found_from_inner_scope() {
    let mut env = SymbolEnvironment::new();

    let outer = ScopeKind::Named(Id::from("FB_Motor").into());
    let inner = ScopeKind::Named(ScopePath::new(vec![
        Id::from("FB_Motor"),
        Id::from("GetSpeed"),
    ]));

    let field = Id::from("speed");
    env.insert(&field, SymbolKind::Variable, &outer).unwrap();

    assert!(
        env.find(&field, &inner).is_some(),
        "an enclosing scope's symbol should be visible from the inner scope"
    );
}

// -----------------------------------------------------------------
// Repeated global declaration names.
// -----------------------------------------------------------------

fn global(env: &mut SymbolEnvironment, name: &str, kind: SymbolKind) -> Result<(), Diagnostic> {
    env.insert(&Id::from(name), kind, &ScopeKind::Global)
}

#[test]
fn insert_when_program_repeats_program_then_p4013_and_first_kept() {
    let mut env = SymbolEnvironment::new();
    let first = Id::from("Main");
    env.insert(&first, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();

    let error = global(&mut env, "main", SymbolKind::Program).unwrap_err();

    assert_eq!(error.code, Problem::PouDeclNameDuplicated.code());
    let kept = env.find(&Id::from("Main"), &ScopeKind::Global).unwrap();
    assert_eq!(kept.span, first.span());
}

#[test]
fn insert_when_program_repeats_function_block_then_p4013() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "T", SymbolKind::FunctionBlock).unwrap();

    let error = global(&mut env, "T", SymbolKind::Program).unwrap_err();

    assert_eq!(error.code, Problem::PouDeclNameDuplicated.code());
}

/// A repeated type or function block is the type environment's to
/// report, so it is recorded here without a second diagnostic.
#[test]
fn insert_when_function_block_repeats_type_then_ok_here() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "T", SymbolKind::Type).unwrap();

    assert!(global(&mut env, "T", SymbolKind::FunctionBlock).is_ok());
}

#[test]
fn insert_when_program_repeats_configuration_then_p4013() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "C", SymbolKind::Configuration).unwrap();

    let error = global(&mut env, "C", SymbolKind::Program).unwrap_err();

    assert_eq!(error.code, Problem::PouDeclNameDuplicated.code());
}

#[test]
fn insert_variable_when_name_repeated_in_scope_then_p4014_and_first_kept() {
    let mut env = SymbolEnvironment::new();
    let scope = ScopeKind::Named(Id::from("Unit").into());
    env.insert_variable(
        &Id::from("x"),
        &scope,
        VariableType::Input,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();

    let error = env
        .insert_variable(
            &Id::from("X"),
            &scope,
            VariableType::Var,
            DeclarationQualifier::Unspecified,
            None,
            None,
        )
        .unwrap_err();

    assert_eq!(error.code, Problem::SymbolDeclDuplicated.code());
    let kept = env.find(&Id::from("x"), &scope).unwrap();
    assert_eq!(kept.kind, SymbolKind::Parameter);
}

#[test]
fn insert_variable_when_same_name_in_two_scopes_then_ok() {
    let mut env = SymbolEnvironment::new();
    env.insert_variable(
        &Id::from("x"),
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();

    assert!(env
        .insert_variable(
            &Id::from("x"),
            &ScopeKind::Named(Id::from("Unit").into()),
            VariableType::Var,
            DeclarationQualifier::Unspecified,
            None,
            None,
        )
        .is_ok());
}

#[test]
fn insert_variable_when_name_is_compiler_provided_then_reserved() {
    let mut env = SymbolEnvironment::new();
    env.insert_compiler_provided(
        &Id::from("__SYSTEM_UP_TIME"),
        SymbolKind::Variable,
        &ScopeKind::Global,
        None,
    )
    .unwrap();

    let error = env
        .insert_variable(
            &Id::from("__SYSTEM_UP_TIME"),
            &ScopeKind::Global,
            VariableType::Global,
            DeclarationQualifier::Unspecified,
            None,
            None,
        )
        .unwrap_err();

    assert_eq!(error.code, Problem::SymbolDeclDuplicated.code());
    assert!(error.primary.message.contains("reserved"));
    assert!(error.secondary.is_empty());
}

/// A global variable and a type of one name are different namespaces
/// as far as this environment is concerned; other rules decide that.
#[test]
fn insert_variable_when_name_matches_type_then_ok() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "T", SymbolKind::Type).unwrap();

    assert!(env
        .insert_variable(
            &Id::from("T"),
            &ScopeKind::Global,
            VariableType::Global,
            DeclarationQualifier::Unspecified,
            None,
            None,
        )
        .is_ok());
}

/// A structure element sharing a declaration's name is not a repeated
/// declaration; those names have their own rules.
#[test]
fn insert_when_structure_element_shares_type_name_then_ok() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "Red", SymbolKind::Type).unwrap();

    assert!(global(&mut env, "Red", SymbolKind::StructureElement).is_ok());
}

#[test]
fn insert_when_name_repeated_in_named_scope_then_p4014() {
    let mut env = SymbolEnvironment::new();
    let scope = ScopeKind::Named(Id::from("Unit").into());
    env.insert(&Id::from("x"), SymbolKind::Variable, &scope)
        .unwrap();

    let error = env
        .insert(&Id::from("x"), SymbolKind::Variable, &scope)
        .unwrap_err();

    assert_eq!(error.code, Problem::SymbolDeclDuplicated.code());
}

/// The innermost declaration of a name wins, so a method local
/// shadows a function block field of the same name.
#[test]
fn find_when_inner_scope_redeclares_name_then_inner_symbol_shadows_outer() {
    let mut env = SymbolEnvironment::new();

    let outer = ScopeKind::Named(Id::from("FB_Motor").into());
    let inner = ScopeKind::Named(ScopePath::new(vec![
        Id::from("FB_Motor"),
        Id::from("GetSpeed"),
    ]));

    let name = Id::from("v");
    env.insert(&name, SymbolKind::Variable, &outer).unwrap();
    env.insert(&name, SymbolKind::Parameter, &inner).unwrap();

    assert_eq!(env.find(&name, &inner).unwrap().kind, SymbolKind::Parameter);
    assert_eq!(env.find(&name, &outer).unwrap().kind, SymbolKind::Variable);
}

/// A name declared only in an inner scope does not leak outward.
#[test]
fn find_when_symbol_is_in_inner_scope_then_not_found_from_enclosing_scope() {
    let mut env = SymbolEnvironment::new();

    let outer = ScopeKind::Named(Id::from("FB_Motor").into());
    let inner = ScopeKind::Named(ScopePath::new(vec![
        Id::from("FB_Motor"),
        Id::from("GetSpeed"),
    ]));

    let local = Id::from("q");
    env.insert(&local, SymbolKind::Variable, &inner).unwrap();

    assert!(env.find(&local, &outer).is_none());
}

/// Enough names that a hash-seeded order almost never matches the
/// declaration order by chance.
fn names(prefix: &str) -> Vec<Id> {
    (0..16).map(|i| Id::from(&format!("{prefix}{i}"))).collect()
}

#[test]
fn get_programs_when_several_declared_then_returns_declaration_order() {
    let mut env = SymbolEnvironment::new();
    let programs = names("prog");
    for name in &programs {
        env.insert(name, SymbolKind::Program, &ScopeKind::Global)
            .unwrap();
    }

    let actual: Vec<&Id> = env.get_programs().into_iter().map(|(id, _)| id).collect();
    let expected: Vec<&Id> = programs.iter().collect();
    assert_eq!(actual, expected);
}

#[test]
fn get_variables_in_scope_when_several_declared_then_returns_declaration_order() {
    let mut env = SymbolEnvironment::new();
    let scope = ScopeKind::Named(Id::from("main").into());
    let variables = names("var");
    for name in &variables {
        env.insert_variable(
            name,
            &scope,
            VariableType::Var,
            DeclarationQualifier::Unspecified,
            None,
            None,
        )
        .unwrap();
    }

    let actual: Vec<&Id> = env
        .get_variables_in_scope(&scope)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let expected: Vec<&Id> = variables.iter().collect();
    assert_eq!(actual, expected);
}

#[test]
fn get_enumeration_values_for_type_when_several_declared_then_returns_declaration_order() {
    let mut env = SymbolEnvironment::new();
    let enum_type = TypeName::from("Color");
    let values = names("value");
    for name in &values {
        env.insert_enumeration_value(name, &enum_type);
    }

    let actual = env.get_enumeration_values_for_type(&enum_type);
    let expected: Vec<&Id> = values.iter().collect();
    assert_eq!(actual, expected);
}

#[test]
fn insert_when_symbol_redefined_then_keeps_first_declared_position() {
    let mut env = SymbolEnvironment::new();
    let first = Id::from("first");
    let second = Id::from("second");
    env.insert(&first, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();
    env.insert(&second, SymbolKind::Program, &ScopeKind::Global)
        .unwrap();
    // The repeat is reported and the first declaration stays in place.
    assert!(env
        .insert(&first, SymbolKind::Program, &ScopeKind::Global)
        .is_err());

    let actual: Vec<&Id> = env.get_programs().into_iter().map(|(id, _)| id).collect();
    assert_eq!(actual, vec![&first, &second]);
}

#[test]
fn get_variables_in_scope_when_global_scope_then_returns_global_variables_only() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "Main", SymbolKind::Program).unwrap();
    global(&mut env, "Speed", SymbolKind::Type).unwrap();
    env.insert_variable(
        &Id::from("shared"),
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();

    let actual: Vec<&Id> = env
        .get_variables_in_scope(&ScopeKind::Global)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(actual, vec![&Id::from("shared")]);
}

#[test]
fn insert_variable_when_constant_qualifier_then_symbol_is_constant() {
    let mut env = SymbolEnvironment::new();
    env.insert_variable(
        &Id::from("limit"),
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Constant,
        None,
        None,
    )
    .unwrap();

    let symbol = env.find(&Id::from("limit"), &ScopeKind::Global).unwrap();
    assert!(symbol.is_constant());
}

#[test]
fn insert_variable_when_retain_qualifier_then_symbol_is_not_constant() {
    let mut env = SymbolEnvironment::new();
    env.insert_variable(
        &Id::from("count"),
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Retain,
        None,
        None,
    )
    .unwrap();

    let symbol = env.find(&Id::from("count"), &ScopeKind::Global).unwrap();
    assert!(!symbol.is_constant());
}

#[test]
fn insert_when_variable_repeats_result_variable_then_replaces_without_diagnostic() {
    let mut env = SymbolEnvironment::new();
    let scope = ScopeKind::Named(Id::from("F").into());
    env.insert(&Id::from("F"), SymbolKind::ResultVariable, &scope)
        .unwrap();

    env.insert_variable(
        &Id::from("F"),
        &scope,
        VariableType::Var,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();

    let symbol = env.find(&Id::from("F"), &scope).unwrap();
    assert_eq!(symbol.kind, SymbolKind::Variable);
}

#[test]
fn get_variables_in_scope_when_result_variable_then_not_listed() {
    let mut env = SymbolEnvironment::new();
    let scope = ScopeKind::Named(Id::from("F").into());
    env.insert(&Id::from("F"), SymbolKind::ResultVariable, &scope)
        .unwrap();

    assert!(env.get_variables_in_scope(&scope).is_empty());
}

#[test]
fn scope_tracker_when_no_scope_entered_then_global() {
    assert_eq!(ScopeTracker::default().current(), ScopeKind::Global);
}

fn function_block_decl(name: &str) -> ironplc_dsl::common::FunctionBlockDeclaration {
    ironplc_dsl::common::FunctionBlockDeclaration {
        name: TypeName::from(name),
        variables: vec![],
        edge_variables: vec![],
        body: ironplc_dsl::common::FunctionBlockBodyKind::empty(),
        span: ironplc_dsl::core::SourceSpan::default(),
        oop: None,
        methods: vec![],
        properties: vec![],
    }
}

fn method_decl(name: &str) -> ironplc_dsl::common::MethodDeclaration {
    ironplc_dsl::common::MethodDeclaration {
        qualifiers: Default::default(),
        name: Id::from(name),
        return_type: None,
        implicit_variables: vec![],
        variables: vec![],
        edge_variables: vec![],
        body: vec![],
        span: ironplc_dsl::core::SourceSpan::default(),
        result: Default::default(),
    }
}

#[test]
fn scope_tracker_when_method_entered_then_path_through_block() {
    let block = function_block_decl("FB_Axis");
    let start = method_decl("Start");
    let mut tracker = ScopeTracker::default();

    tracker.enter(&ScopeNode::FunctionBlock(&block));
    tracker.enter(&ScopeNode::Method(&start));

    assert_eq!(
        tracker.current(),
        ScopeKind::Named(ScopePath::new(vec![Id::from("FB_Axis"), Id::from("Start")]))
    );
    assert_eq!(tracker.unit(), Some(&Id::from("FB_Axis")));
}

#[test]
fn scope_tracker_when_exited_then_unit_none() {
    let block = function_block_decl("FB_Axis");
    let mut tracker = ScopeTracker::default();

    tracker.enter(&ScopeNode::FunctionBlock(&block));
    tracker.exit();

    assert_eq!(tracker.unit(), None);
    assert_eq!(tracker.current(), ScopeKind::Global);
}

#[test]
fn scope_of_when_method_not_entered_then_scope_it_would_open() {
    let block = function_block_decl("FB_Axis");
    let start = method_decl("Start");
    let mut tracker = ScopeTracker::default();
    tracker.enter(&ScopeNode::FunctionBlock(&block));

    let scope = tracker.scope_of(&ScopeNode::Method(&start));

    tracker.enter(&ScopeNode::Method(&start));
    assert_eq!(scope, tracker.current());
}

fn function_block(env: &mut SymbolEnvironment, name: &str, extends: Option<&str>) {
    let info = SymbolInfo::new(
        SymbolKind::FunctionBlock,
        ScopeKind::Global,
        ironplc_dsl::core::SourceSpan::default(),
    )
    .with_extends(extends.map(TypeName::from));
    env.insert_info(&Id::from(name), info).unwrap();
}

fn field(env: &mut SymbolEnvironment, block: &str, name: &str) {
    env.insert_variable(
        &Id::from(name),
        &ScopeKind::Named(Id::from(block).into()),
        VariableType::Var,
        DeclarationQualifier::Unspecified,
        None,
        None,
    )
    .unwrap();
}

#[test]
fn find_when_field_declared_on_base_then_visible_from_derived_method() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Mid", Some("Base"));
    function_block(&mut env, "Derived", Some("Mid"));
    field(&mut env, "Base", "speed");
    let method = ScopeKind::Named(ScopePath::new(vec![Id::from("Derived"), Id::from("M")]));

    let symbol = env.find(&Id::from("speed"), &method).unwrap();

    assert_eq!(symbol.scope, ScopeKind::Named(Id::from("Base").into()));
}

#[test]
fn find_when_derived_redeclares_base_field_then_derived_wins() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Derived", Some("Base"));
    field(&mut env, "Base", "speed");
    field(&mut env, "Derived", "speed");
    let derived = ScopeKind::Named(Id::from("Derived").into());

    let symbol = env.find(&Id::from("speed"), &derived).unwrap();

    assert_eq!(symbol.scope, derived);
}

#[test]
fn find_when_extends_cycle_then_terminates() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "A", Some("B"));
    function_block(&mut env, "B", Some("A"));

    assert!(env
        .find(
            &Id::from("missing"),
            &ScopeKind::Named(Id::from("A").into())
        )
        .is_none());
}

#[test]
fn visible_variables_when_inherited_then_listed_once_nearest_first() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Derived", Some("Base"));
    field(&mut env, "Base", "speed");
    field(&mut env, "Base", "limit");
    field(&mut env, "Derived", "speed");

    let visible = env.visible_variables(&ScopeKind::Named(Id::from("Derived").into()));
    let names: Vec<String> = visible.iter().map(|(name, _)| name.to_string()).collect();

    assert_eq!(names, vec!["speed", "limit"]);
}

fn self_types(env: &SymbolEnvironment, scope: &ScopeKind) -> (Option<String>, Option<String>) {
    let name = |kind| env.self_type(scope, kind).map(|t: TypeName| t.to_string());
    (name(SelfRefKind::This), name(SelfRefKind::Super))
}

#[test]
fn self_type_when_method_of_derived_block_then_block_and_base() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);
    function_block(&mut env, "Derived", Some("Base"));
    let method = ScopeKind::Named(ScopePath::new(vec![Id::from("Derived"), Id::from("M")]));

    assert_eq!(
        (Some("Derived".to_string()), Some("Base".to_string())),
        self_types(&env, &method)
    );
}

#[test]
fn self_type_when_block_without_base_then_super_none() {
    let mut env = SymbolEnvironment::new();
    function_block(&mut env, "Base", None);

    assert_eq!(
        (Some("Base".to_string()), None),
        self_types(&env, &ScopeKind::Named(Id::from("Base").into()))
    );
}

#[test]
fn self_type_when_program_or_global_then_none() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "main", SymbolKind::Program).unwrap();

    assert_eq!(
        (None, None),
        self_types(&env, &ScopeKind::Named(Id::from("main").into()))
    );
    assert_eq!((None, None), self_types(&env, &ScopeKind::Global));
}
