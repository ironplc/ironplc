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
fn get_enumeration_values_for_type_when_values_in_global_and_scoped_then_returns_matching_only() {
    let mut env = SymbolEnvironment::new();
    let enum_type = TypeName::from("COLOR");
    let other_type = TypeName::from("SIZE");

    // Global enumeration value of the requested type.
    env.insert_enumeration_value(&Id::from("RED"), &enum_type, &ScopeKind::Global)
        .unwrap();
    // Scoped enumeration value of the requested type.
    let scope = ScopeKind::Named(Id::from("FB").into());
    env.insert_enumeration_value(&Id::from("GREEN"), &enum_type, &scope)
        .unwrap();
    // Enumeration value of a different type (should be excluded).
    env.insert_enumeration_value(&Id::from("SMALL"), &other_type, &ScopeKind::Global)
        .unwrap();
    // Non-enumeration symbol whose enum_type is None (should be excluded).
    env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let values = env.get_enumeration_values_for_type(&enum_type);
    assert_eq!(values.len(), 2);
    assert!(values.iter().any(|id| **id == Id::from("RED")));
    assert!(values.iter().any(|id| **id == Id::from("GREEN")));
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
fn get_structure_fields_for_type_when_fields_in_global_and_scoped_then_returns_matching_only() {
    let mut env = SymbolEnvironment::new();
    let struct_type = TypeName::from("POINT");
    let other_type = TypeName::from("LINE");

    // Global structure field of the requested type.
    env.insert_structure_field(&Id::from("X"), &struct_type, &ScopeKind::Global)
        .unwrap();
    // Scoped structure field of the requested type.
    let scope = ScopeKind::Named(Id::from("FB").into());
    env.insert_structure_field(&Id::from("Y"), &struct_type, &scope)
        .unwrap();
    // Structure field of a different type (should be excluded).
    env.insert_structure_field(&Id::from("START"), &other_type, &ScopeKind::Global)
        .unwrap();
    // Non-structure symbol whose struct_type is None (should be excluded).
    env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let fields = env.get_structure_fields_for_type(&struct_type);
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().any(|id| **id == Id::from("X")));
    assert!(fields.iter().any(|id| **id == Id::from("Y")));
}

#[test]
fn get_structure_fields_for_type_when_no_matching_fields_then_returns_empty() {
    let mut env = SymbolEnvironment::new();
    env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
        .unwrap();

    let fields = env.get_structure_fields_for_type(&TypeName::from("POINT"));
    assert!(fields.is_empty());
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
    assert!(symbol_info.data_type.is_none());
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
        SymbolKind::Parameter,
        &scope,
        VariableType::Input,
        DeclarationQualifier::Unspecified,
        None,
    )
    .unwrap();

    let error = env
        .insert_variable(
            &Id::from("X"),
            SymbolKind::Variable,
            &scope,
            VariableType::Var,
            DeclarationQualifier::Unspecified,
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
        SymbolKind::Variable,
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Unspecified,
        None,
    )
    .unwrap();

    assert!(env
        .insert_variable(
            &Id::from("x"),
            SymbolKind::Variable,
            &ScopeKind::Named(Id::from("Unit").into()),
            VariableType::Var,
            DeclarationQualifier::Unspecified,
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
    )
    .unwrap();

    let error = env
        .insert_variable(
            &Id::from("__SYSTEM_UP_TIME"),
            SymbolKind::Variable,
            &ScopeKind::Global,
            VariableType::Global,
            DeclarationQualifier::Unspecified,
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
            SymbolKind::Variable,
            &ScopeKind::Global,
            VariableType::Global,
            DeclarationQualifier::Unspecified,
            None,
        )
        .is_ok());
}

/// An enumeration value or structure element sharing a declaration's
/// name is not a repeated declaration; those names have their own rules.
#[test]
fn insert_when_enumeration_value_shares_type_name_then_ok() {
    let mut env = SymbolEnvironment::new();
    global(&mut env, "Red", SymbolKind::Type).unwrap();

    assert!(global(&mut env, "Red", SymbolKind::EnumerationValue).is_ok());
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
            SymbolKind::Variable,
            &scope,
            VariableType::Var,
            DeclarationQualifier::Unspecified,
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
        env.insert_enumeration_value(name, &enum_type, &ScopeKind::Global)
            .unwrap();
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
        SymbolKind::Variable,
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Unspecified,
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
        SymbolKind::Variable,
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Constant,
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
        SymbolKind::Variable,
        &ScopeKind::Global,
        VariableType::Global,
        DeclarationQualifier::Retain,
        None,
    )
    .unwrap();

    let symbol = env.find(&Id::from("count"), &ScopeKind::Global).unwrap();
    assert!(!symbol.is_constant());
}
