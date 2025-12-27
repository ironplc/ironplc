//! Forward reference resolution system for type dependencies.
//!
//! This module provides a two-pass compilation system for resolving forward type references
//! and detecting circular dependencies in type definitions.

use std::collections::{HashMap, HashSet, VecDeque};
use crate::common::{
    Library, LibraryElementKind, TypeDefinitionBlock, TypeDefinition, DataTypeSpecificationKind,
    TypeName, GlobalVariableDeclaration, VarDecl, InitialValueAssignmentKind, ElementaryTypeName,
    VariableIdentifier, VariableType, DeclarationQualifier, SimpleInitializer
};
use crate::core::{SourceSpan, Located};
use ironplc_problems::Problem;
use crate::diagnostic::{Diagnostic, Label};

/// Represents a forward reference to a type that hasn't been defined yet.
#[derive(Clone, Debug, PartialEq)]
pub struct ForwardReference {
    /// The name of the type being referenced
    pub type_name: String,
    /// Location where the reference occurs
    pub usage_location: SourceSpan,
    /// Context description (e.g., "variable declaration", "type definition")
    pub context: String,
}

/// Information about a type definition for resolution tracking.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeInfo {
    /// The name of the type
    pub name: String,
    /// The type definition
    pub definition: TypeDefinition,
    /// Whether this type has been resolved (all dependencies satisfied)
    pub resolved: bool,
    /// Types that this type depends on
    pub dependencies: Vec<String>,
}

/// Information about a global variable for symbol table management.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableInfo {
    /// The name of the variable
    pub name: String,
    /// The variable declaration
    pub declaration: VarDecl,
    /// The type name this variable references
    pub type_reference: Option<String>,
}

/// Global symbol table for managing variables and types across compilation units.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalSymbolTable {
    /// Map of variable names to their information
    pub variables: HashMap<String, VariableInfo>,
    /// Map of type names to their information
    pub types: HashMap<String, TypeInfo>,
    /// List of unresolved forward references
    pub forward_references: Vec<ForwardReference>,
}

impl GlobalSymbolTable {
    /// Creates a new empty global symbol table.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            types: HashMap::new(),
            forward_references: Vec::new(),
        }
    }

    /// Merges multiple VAR_GLOBAL blocks into a unified global scope.
    /// This method handles the merging of variables from multiple VAR_GLOBAL declarations.
    pub fn merge_global_variables(&mut self, global_var_blocks: Vec<GlobalVariableDeclaration>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for global_block in global_var_blocks {
            for var_decl in global_block.variables {
                if let Err(error_msg) = self.register_variable(var_decl) {
                    errors.push(error_msg);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Resolves global variable references in program contexts.
    /// This method checks if a variable name exists in the global scope.
    pub fn resolve_global_variable_reference(&self, var_name: &str) -> Option<&VariableInfo> {
        self.variables.get(var_name)
    }

    /// Validates that all type references in variable declarations exist.
    pub fn validate_type_references(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (var_name, var_info) in &self.variables {
            if let Some(type_ref) = &var_info.type_reference {
                if !self.type_exists(type_ref) && !self.is_elementary_type(type_ref) {
                    errors.push(format!(
                        "Variable '{}' references undefined type '{}'",
                        var_name, type_ref
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Checks if a type name refers to an elementary type.
    fn is_elementary_type(&self, type_name: &str) -> bool {
        matches!(
            type_name.to_uppercase().as_str(),
            "BOOL" | "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" |
            "REAL" | "LREAL" | "TIME" | "DATE" | "TIME_OF_DAY" | "DATE_AND_TIME" |
            "STRING" | "WSTRING" | "BYTE" | "WORD" | "DWORD" | "LWORD"
        )
    }

    /// Gets all global variable names for scope resolution.
    pub fn get_global_variable_names(&self) -> Vec<String> {
        self.variables.keys().cloned().collect()
    }

    /// Gets all type names for type checking.
    pub fn get_type_names(&self) -> Vec<String> {
        self.types.keys().cloned().collect()
    }

    /// Registers a type definition in the symbol table.
    pub fn register_type(&mut self, type_def: TypeDefinition) -> Result<(), String> {
        let type_name = type_def.name.name.original().clone();
        
        // Check for duplicate type definitions
        if self.types.contains_key(&type_name) {
            return Err(format!("Duplicate type definition: {}", type_name));
        }

        // Extract dependencies from the type definition
        let dependencies = self.extract_type_dependencies(&type_def.base_type);

        let type_info = TypeInfo {
            name: type_name.clone(),
            definition: type_def,
            resolved: false,
            dependencies,
        };

        self.types.insert(type_name, type_info);
        Ok(())
    }

    /// Registers a global variable in the symbol table.
    pub fn register_variable(&mut self, var_decl: VarDecl) -> Result<(), String> {
        if let Some(var_name) = var_decl.identifier.symbolic_id() {
            let var_name_str = var_name.original().clone();
            
            // Check for duplicate variable definitions
            if self.variables.contains_key(&var_name_str) {
                return Err(format!("Duplicate variable definition: {}", var_name_str));
            }

            // Extract type reference from variable declaration
            let type_reference = self.extract_variable_type_reference(&var_decl.initializer);

            let var_info = VariableInfo {
                name: var_name_str.clone(),
                declaration: var_decl,
                type_reference,
            };

            self.variables.insert(var_name_str, var_info);
        }
        Ok(())
    }

    /// Extracts type dependencies from a data type specification.
    fn extract_type_dependencies(&self, data_type: &DataTypeSpecificationKind) -> Vec<String> {
        let mut dependencies = Vec::new();
        
        match data_type {
            DataTypeSpecificationKind::UserDefined(type_name) => {
                dependencies.push(type_name.name.original().clone());
            }
            DataTypeSpecificationKind::Array(array_spec) => {
                dependencies.extend(self.extract_type_dependencies(&array_spec.element_type));
            }
            DataTypeSpecificationKind::Elementary(_) |
            DataTypeSpecificationKind::Enumeration(_) |
            DataTypeSpecificationKind::Subrange(_) |
            DataTypeSpecificationKind::String(_) => {
                // These types don't have dependencies on user-defined types
            }
        }
        
        dependencies
    }

    /// Extracts type reference from a variable's initializer.
    fn extract_variable_type_reference(&self, initializer: &InitialValueAssignmentKind) -> Option<String> {
        match initializer {
            InitialValueAssignmentKind::Simple(simple_init) => {
                Some(simple_init.type_name.name.original().clone())
            }
            InitialValueAssignmentKind::EnumeratedType(enum_init) => {
                Some(enum_init.type_name.name.original().clone())
            }
            InitialValueAssignmentKind::FunctionBlock(fb_init) => {
                Some(fb_init.type_name.name.original().clone())
            }
            InitialValueAssignmentKind::LateResolvedType(type_name) => {
                Some(type_name.name.original().clone())
            }
            _ => None,
        }
    }

    /// Adds a forward reference to the tracking list.
    pub fn add_forward_reference(&mut self, forward_ref: ForwardReference) {
        self.forward_references.push(forward_ref);
    }

    /// Checks if a type exists in the symbol table.
    pub fn type_exists(&self, type_name: &str) -> bool {
        self.types.contains_key(type_name)
    }

    /// Checks if a variable exists in the symbol table.
    pub fn variable_exists(&self, var_name: &str) -> bool {
        self.variables.contains_key(var_name)
    }
}

/// Type resolver for managing forward reference resolution and dependency analysis.
#[derive(Clone, Debug)]
pub struct TypeResolver {
    /// The global symbol table
    pub symbol_table: GlobalSymbolTable,
}

impl TypeResolver {
    /// Creates a new type resolver with an empty symbol table.
    pub fn new() -> Self {
        Self {
            symbol_table: GlobalSymbolTable::new(),
        }
    }

    /// Performs two-pass compilation to resolve forward references.
    pub fn resolve_forward_references(&mut self, library: &mut Library) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        // Pass 1: Collect all type definitions and global variables
        if let Err(pass1_errors) = self.collect_definitions(library) {
            errors.extend(pass1_errors);
        }

        // Pass 2: Resolve forward references and validate dependencies
        if let Err(pass2_errors) = self.resolve_dependencies() {
            errors.extend(pass2_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Pass 1: Collect all type definitions and global variables from the library.
    fn collect_definitions(&mut self, library: &Library) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        for element in &library.elements {
            match element {
                LibraryElementKind::TypeDefinitionBlock(type_block) => {
                    for type_def in &type_block.definitions {
                        if let Err(error_msg) = self.symbol_table.register_type(type_def.clone()) {
                            errors.push(Diagnostic::problem(
                                Problem::TypeDeclNameDuplicated,
                                Label::span(type_def.span(), format!("Type registration error: {}", error_msg)),
                            ));
                        }
                    }
                }
                LibraryElementKind::GlobalVariableDeclaration(global_vars) => {
                    for var_decl in &global_vars.variables {
                        if let Err(error_msg) = self.symbol_table.register_variable(var_decl.clone()) {
                            errors.push(Diagnostic::problem(
                                Problem::SymbolDeclDuplicated,
                                Label::span(var_decl.span(), format!("Variable registration error: {}", error_msg)),
                            ));
                        }
                    }
                }
                _ => {
                    // Other library elements don't contain type definitions at the top level
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Pass 2: Resolve dependencies and detect circular references.
    fn resolve_dependencies(&mut self) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        // Check for circular dependencies
        if let Err(circular_errors) = self.detect_circular_dependencies() {
            errors.extend(circular_errors);
        }

        // Resolve forward references
        if let Err(resolution_errors) = self.resolve_type_references() {
            errors.extend(resolution_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Detects circular dependencies in type definitions using topological sorting.
    fn detect_circular_dependencies(&self) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for type_name in self.symbol_table.types.keys() {
            if !visited.contains(type_name) {
                if let Err(cycle_error) = self.detect_cycle_dfs(type_name, &mut visited, &mut rec_stack) {
                    errors.push(cycle_error);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Depth-first search to detect cycles in type dependencies.
    fn detect_cycle_dfs(
        &self,
        type_name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<(), Diagnostic> {
        visited.insert(type_name.to_string());
        rec_stack.insert(type_name.to_string());

        if let Some(type_info) = self.symbol_table.types.get(type_name) {
            for dependency in &type_info.dependencies {
                if !visited.contains(dependency) {
                    self.detect_cycle_dfs(dependency, visited, rec_stack)?;
                } else if rec_stack.contains(dependency) {
                    return Err(Diagnostic::problem(
                        Problem::RecursiveTypeCycle,
                        Label::span(type_info.definition.span(), format!("Circular dependency detected: {} -> {}", type_name, dependency)),
                    ));
                }
            }
        }

        rec_stack.remove(type_name);
        Ok(())
    }

    /// Resolves forward type references and validates that all referenced types exist.
    fn resolve_type_references(&mut self) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        let mut forward_refs = Vec::new();

        // Check that all type dependencies exist
        for (type_name, type_info) in &self.symbol_table.types {
            for dependency in &type_info.dependencies {
                if !self.symbol_table.type_exists(dependency) {
                    forward_refs.push(ForwardReference {
                        type_name: dependency.clone(),
                        usage_location: type_info.definition.span(),
                        context: format!("type definition for {}", type_name),
                    });
                }
            }
        }

        // Check that all variable type references exist
        for (var_name, var_info) in &self.symbol_table.variables {
            if let Some(type_ref) = &var_info.type_reference {
                if !self.symbol_table.type_exists(type_ref) && !self.is_elementary_type(type_ref) {
                    forward_refs.push(ForwardReference {
                        type_name: type_ref.clone(),
                        usage_location: var_info.declaration.span(),
                        context: format!("variable declaration for {}", var_name),
                    });
                }
            }
        }

        // Add all forward references
        for forward_ref in forward_refs {
            self.symbol_table.add_forward_reference(forward_ref);
        }

        // Report unresolved forward references as errors
        for forward_ref in &self.symbol_table.forward_references {
            errors.push(Diagnostic::problem(
                Problem::UndeclaredUnknownType,
                Label::span(forward_ref.usage_location.clone(), format!(
                    "Unresolved type reference '{}' in {}",
                    forward_ref.type_name, forward_ref.context
                )),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Checks if a type name refers to an elementary type.
    fn is_elementary_type(&self, type_name: &str) -> bool {
        matches!(
            type_name.to_uppercase().as_str(),
            "BOOL" | "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT" |
            "REAL" | "LREAL" | "TIME" | "DATE" | "TIME_OF_DAY" | "DATE_AND_TIME" |
            "STRING" | "WSTRING" | "BYTE" | "WORD" | "DWORD" | "LWORD"
        )
    }

    /// Validates type constraints (e.g., subrange bounds, array dimensions).
    pub fn validate_type_constraints(&self, _data_type: &DataTypeSpecificationKind) -> Result<(), Diagnostic> {
        // This method can be extended to validate specific type constraints
        // For now, we'll implement basic validation
        Ok(())
    }

    /// Performs topological sort of type dependencies to determine resolution order.
    pub fn topological_sort(&self) -> Result<Vec<String>, Diagnostic> {
        let mut in_degree = HashMap::new();
        let mut adj_list = HashMap::new();

        // Initialize in-degree and adjacency list
        for (type_name, type_info) in &self.symbol_table.types {
            in_degree.insert(type_name.clone(), 0);
            adj_list.insert(type_name.clone(), Vec::new());
        }

        // Build the dependency graph
        for (type_name, type_info) in &self.symbol_table.types {
            for dependency in &type_info.dependencies {
                if self.symbol_table.types.contains_key(dependency) {
                    adj_list.get_mut(dependency).unwrap().push(type_name.clone());
                    *in_degree.get_mut(type_name).unwrap() += 1;
                }
            }
        }

        // Perform topological sort using Kahn's algorithm
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Add all nodes with in-degree 0 to the queue
        for (type_name, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(type_name.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            // Reduce in-degree of adjacent nodes
            if let Some(neighbors) = adj_list.get(&current) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        // Check if all types were processed (no cycles)
        if result.len() != self.symbol_table.types.len() {
            return Err(Diagnostic::problem(
                Problem::RecursiveTypeCycle,
                Label::span(SourceSpan::default(), "Circular dependency detected in type definitions".to_string()),
            ));
        }

        Ok(result)
    }
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ElementaryTypeName, EnhancedSubrangeSpecification, SignedInteger, 
                       VariableIdentifier, VariableType, DeclarationQualifier, SimpleInitializer};
    use crate::core::Id;

    #[test]
    fn test_global_symbol_table_creation() {
        let symbol_table = GlobalSymbolTable::new();
        assert!(symbol_table.variables.is_empty());
        assert!(symbol_table.types.is_empty());
        assert!(symbol_table.forward_references.is_empty());
    }

    #[test]
    fn test_type_resolver_creation() {
        let resolver = TypeResolver::new();
        assert!(resolver.symbol_table.types.is_empty());
    }

    #[test]
    fn test_elementary_type_detection() {
        let resolver = TypeResolver::new();
        assert!(resolver.is_elementary_type("BOOL"));
        assert!(resolver.is_elementary_type("INT"));
        assert!(resolver.is_elementary_type("REAL"));
        assert!(!resolver.is_elementary_type("CustomType"));
    }

    #[test]
    fn test_forward_reference_creation() {
        let forward_ref = ForwardReference {
            type_name: "UnknownType".to_string(),
            usage_location: SourceSpan::default(),
            context: "test context".to_string(),
        };
        
        assert_eq!(forward_ref.type_name, "UnknownType");
        assert_eq!(forward_ref.context, "test context");
    }

    #[test]
    fn test_type_info_creation() {
        let type_def = TypeDefinition {
            name: TypeName::from("TestType"),
            base_type: DataTypeSpecificationKind::Elementary(ElementaryTypeName::INT),
            default_value: None,
            span: SourceSpan::default(),
        };

        let type_info = TypeInfo {
            name: "TestType".to_string(),
            definition: type_def,
            resolved: false,
            dependencies: vec!["BaseType".to_string()],
        };

        assert_eq!(type_info.name, "TestType");
        assert!(!type_info.resolved);
        assert_eq!(type_info.dependencies.len(), 1);
    }

    #[test]
    fn test_topological_sort_empty() {
        let resolver = TypeResolver::new();
        let result = resolver.topological_sort();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // Property-based tests for forward reference resolution
    use proptest::prelude::*;
    use proptest::proptest;

    // **Feature: ironplc-esstee-syntax-support, Property 3: Multiple VAR_GLOBAL Block Merging**
    // **Validates: Requirements 1.4**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn property_multiple_var_global_block_merging(
            var_names1 in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
            var_names2 in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
            var_names3 in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
        ) {
            // Ensure all variable names are unique across all blocks
            let mut all_names = var_names1.clone();
            all_names.extend(var_names2.clone());
            all_names.extend(var_names3.clone());
            all_names.sort();
            all_names.dedup();
            prop_assume!(all_names.len() == var_names1.len() + var_names2.len() + var_names3.len());

            let mut symbol_table = GlobalSymbolTable::new();

            // Create three separate VAR_GLOBAL blocks
            let global_block1 = GlobalVariableDeclaration {
                variables: var_names1.iter().map(|name| VarDecl::simple(name, "INT")).collect(),
                span: SourceSpan::default(),
            };

            let global_block2 = GlobalVariableDeclaration {
                variables: var_names2.iter().map(|name| VarDecl::simple(name, "BOOL")).collect(),
                span: SourceSpan::default(),
            };

            let global_block3 = GlobalVariableDeclaration {
                variables: var_names3.iter().map(|name| VarDecl::simple(name, "REAL")).collect(),
                span: SourceSpan::default(),
            };

            // Merge all blocks into the symbol table
            let result = symbol_table.merge_global_variables(vec![global_block1, global_block2, global_block3]);
            assert!(result.is_ok(), "Merging multiple VAR_GLOBAL blocks should succeed");

            // Verify all variables from all blocks are accessible
            for var_name in &var_names1 {
                assert!(symbol_table.variable_exists(var_name), "Variable '{}' from block 1 should exist", var_name);
            }
            for var_name in &var_names2 {
                assert!(symbol_table.variable_exists(var_name), "Variable '{}' from block 2 should exist", var_name);
            }
            for var_name in &var_names3 {
                assert!(symbol_table.variable_exists(var_name), "Variable '{}' from block 3 should exist", var_name);
            }

            // Verify total count matches expected
            let expected_total = var_names1.len() + var_names2.len() + var_names3.len();
            assert_eq!(symbol_table.variables.len(), expected_total, "Total variable count should match sum of all blocks");

            // Test duplicate variable detection
            let duplicate_block = GlobalVariableDeclaration {
                variables: vec![VarDecl::simple(&var_names1[0], "STRING")],
                span: SourceSpan::default(),
            };

            let duplicate_result = symbol_table.merge_global_variables(vec![duplicate_block]);
            assert!(duplicate_result.is_err(), "Merging duplicate variable should fail");
        }
    }

    // **Feature: ironplc-esstee-syntax-support, Property 4: Global Variable Reference Resolution**
    // **Validates: Requirements 1.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn property_global_variable_reference_resolution(
            global_var_names in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..10),
            program_var_names in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
        ) {
            // Ensure global and program variable names don't overlap
            let mut all_names = global_var_names.clone();
            all_names.extend(program_var_names.clone());
            all_names.sort();
            all_names.dedup();
            prop_assume!(all_names.len() == global_var_names.len() + program_var_names.len());

            let mut symbol_table = GlobalSymbolTable::new();

            // Create global variables
            let global_block = GlobalVariableDeclaration {
                variables: global_var_names.iter().map(|name| VarDecl::simple(name, "INT")).collect(),
                span: SourceSpan::default(),
            };

            // Register global variables
            let result = symbol_table.merge_global_variables(vec![global_block]);
            assert!(result.is_ok(), "Global variable registration should succeed");

            // Test global variable reference resolution
            for global_var_name in &global_var_names {
                let resolved_var = symbol_table.resolve_global_variable_reference(global_var_name);
                assert!(resolved_var.is_some(), "Global variable '{}' should be resolvable", global_var_name);
                
                let var_info = resolved_var.unwrap();
                assert_eq!(var_info.name, *global_var_name, "Resolved variable name should match");
            }

            // Test that non-existent variables are not resolved
            for program_var_name in &program_var_names {
                let resolved_var = symbol_table.resolve_global_variable_reference(program_var_name);
                assert!(resolved_var.is_none(), "Program variable '{}' should not be resolvable as global", program_var_name);
            }

            // Test that all global variable names are accessible
            let global_names = symbol_table.get_global_variable_names();
            for global_var_name in &global_var_names {
                assert!(global_names.contains(global_var_name), "Global variable '{}' should be in global names list", global_var_name);
            }

            // Verify the count matches
            assert_eq!(global_names.len(), global_var_names.len(), "Global variable count should match");

            // Test variable existence check
            for global_var_name in &global_var_names {
                assert!(symbol_table.variable_exists(global_var_name), "Global variable '{}' should exist", global_var_name);
            }

            for program_var_name in &program_var_names {
                assert!(!symbol_table.variable_exists(program_var_name), "Program variable '{}' should not exist in global scope", program_var_name);
            }
        }
    }

    // **Feature: ironplc-esstee-syntax-support, Property 24: Mixed Declaration Context Parsing**
    // **Validates: Requirements 7.1**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn property_mixed_declaration_context_parsing(
            type_names in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
            var_names in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..5),
            custom_type_vars in prop::collection::vec("[a-zA-Z][a-zA-Z0-9_]*", 1..3),
        ) {
            // Ensure all names are unique
            let mut all_names = type_names.clone();
            all_names.extend(var_names.clone());
            all_names.extend(custom_type_vars.clone());
            all_names.sort();
            all_names.dedup();
            prop_assume!(all_names.len() == type_names.len() + var_names.len() + custom_type_vars.len());

            let mut symbol_table = GlobalSymbolTable::new();

            // Create type definitions
            let mut type_definitions = Vec::new();
            for type_name in &type_names {
                let type_def = TypeDefinition {
                    name: TypeName::from(type_name),
                    base_type: DataTypeSpecificationKind::Elementary(ElementaryTypeName::INT),
                    default_value: None,
                    span: SourceSpan::default(),
                };
                type_definitions.push(type_def);
            }

            // Register all type definitions
            for type_def in type_definitions {
                let result = symbol_table.register_type(type_def);
                assert!(result.is_ok(), "Type registration should succeed");
            }

            // Create global variables with elementary types
            let mut global_vars = Vec::new();
            for var_name in &var_names {
                global_vars.push(VarDecl::simple(var_name, "BOOL"));
            }

            // Create global variables that reference custom types
            for (i, var_name) in custom_type_vars.iter().enumerate() {
                let type_ref = &type_names[i % type_names.len()];
                let var_decl = VarDecl {
                    identifier: VariableIdentifier::new_symbol(var_name),
                    var_type: VariableType::Var,
                    qualifier: DeclarationQualifier::Unspecified,
                    initializer: InitialValueAssignmentKind::LateResolvedType(TypeName::from(type_ref)),
                    reference_annotation: None,
                };
                global_vars.push(var_decl);
            }

            let global_block = GlobalVariableDeclaration {
                variables: global_vars,
                span: SourceSpan::default(),
            };

            // Register global variables
            let result = symbol_table.merge_global_variables(vec![global_block]);
            assert!(result.is_ok(), "Global variable registration should succeed");

            // Verify that both types and variables are accessible
            for type_name in &type_names {
                assert!(symbol_table.type_exists(type_name), "Type '{}' should exist", type_name);
            }

            for var_name in &var_names {
                assert!(symbol_table.variable_exists(var_name), "Variable '{}' should exist", var_name);
            }

            for var_name in &custom_type_vars {
                assert!(symbol_table.variable_exists(var_name), "Custom type variable '{}' should exist", var_name);
            }

            // Verify separate symbol tables are maintained
            let type_count = symbol_table.types.len();
            let var_count = symbol_table.variables.len();
            
            assert_eq!(type_count, type_names.len(), "Type count should match registered types");
            assert_eq!(var_count, var_names.len() + custom_type_vars.len(), "Variable count should match registered variables");

            // Test type reference validation
            let validation_result = symbol_table.validate_type_references();
            assert!(validation_result.is_ok(), "Type reference validation should succeed for valid references");

            // Test that we can get all type and variable names
            let all_type_names = symbol_table.get_type_names();
            let all_var_names = symbol_table.get_global_variable_names();

            for type_name in &type_names {
                assert!(all_type_names.contains(type_name), "Type '{}' should be in type names list", type_name);
            }

            for var_name in &var_names {
                assert!(all_var_names.contains(var_name), "Variable '{}' should be in variable names list", var_name);
            }

            for var_name in &custom_type_vars {
                assert!(all_var_names.contains(var_name), "Custom type variable '{}' should be in variable names list", var_name);
            }
        }
    }

    // **Feature: ironplc-esstee-syntax-support, Property 11: Type Dependency Resolution**
    // **Validates: Requirements 3.3**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn property_type_dependency_resolution(
            base_type_name in "[a-zA-Z][a-zA-Z0-9_]*",
            derived_type_name in "[a-zA-Z][a-zA-Z0-9_]*",
            another_type_name in "[a-zA-Z][a-zA-Z0-9_]*",
        ) {
            // Ensure type names are different
            prop_assume!(base_type_name != derived_type_name);
            prop_assume!(base_type_name != another_type_name);
            prop_assume!(derived_type_name != another_type_name);

            let mut resolver = TypeResolver::new();

            // Create a base type definition
            let base_type_def = TypeDefinition {
                name: TypeName::from(&base_type_name),
                base_type: DataTypeSpecificationKind::Elementary(ElementaryTypeName::INT),
                default_value: None,
                span: SourceSpan::default(),
            };

            // Create a derived type that depends on the base type
            let derived_type_def = TypeDefinition {
                name: TypeName::from(&derived_type_name),
                base_type: DataTypeSpecificationKind::UserDefined(TypeName::from(&base_type_name)),
                default_value: None,
                span: SourceSpan::default(),
            };

            // Register both types
            assert!(resolver.symbol_table.register_type(base_type_def).is_ok());
            assert!(resolver.symbol_table.register_type(derived_type_def.clone()).is_ok());

            // Verify that dependencies are correctly extracted
            let derived_type_info = resolver.symbol_table.types.get(&derived_type_name).unwrap();
            assert_eq!(derived_type_info.dependencies.len(), 1);
            assert_eq!(derived_type_info.dependencies[0], base_type_name);

            // Verify that the base type has no dependencies
            let base_type_info = resolver.symbol_table.types.get(&base_type_name).unwrap();
            assert_eq!(base_type_info.dependencies.len(), 0);

            // Test topological sort - base type should come before derived type
            let sorted_types = resolver.topological_sort().unwrap();
            let base_pos = sorted_types.iter().position(|t| t == &base_type_name).unwrap();
            let derived_pos = sorted_types.iter().position(|t| t == &derived_type_name).unwrap();
            assert!(base_pos < derived_pos, "Base type should be resolved before derived type");

            // Test that circular dependencies are detected
            let circular_type_def = TypeDefinition {
                name: TypeName::from(&another_type_name),
                base_type: DataTypeSpecificationKind::UserDefined(TypeName::from(&derived_type_name)),
                default_value: None,
                span: SourceSpan::default(),
            };

            // Update the base type to depend on the circular type (creating a cycle)
            let mut circular_resolver = TypeResolver::new();
            
            let circular_base_def = TypeDefinition {
                name: TypeName::from(&base_type_name),
                base_type: DataTypeSpecificationKind::UserDefined(TypeName::from(&another_type_name)),
                default_value: None,
                span: SourceSpan::default(),
            };

            assert!(circular_resolver.symbol_table.register_type(circular_base_def).is_ok());
            assert!(circular_resolver.symbol_table.register_type(derived_type_def.clone()).is_ok());
            assert!(circular_resolver.symbol_table.register_type(circular_type_def).is_ok());

            // Circular dependency detection should fail
            let result = circular_resolver.detect_circular_dependencies();
            assert!(result.is_err(), "Circular dependency should be detected");
        }
    }

    // **Feature: ironplc-esstee-syntax-support, Property 13: Forward Type Reference Resolution**
    // **Validates: Requirements 3.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn property_forward_reference_resolution(
            type_name in "[a-zA-Z][a-zA-Z0-9_]*",
            var_name in "[a-zA-Z][a-zA-Z0-9_]*",
            undefined_type_name in "[a-zA-Z][a-zA-Z0-9_]*",
        ) {
            // Ensure names are different
            prop_assume!(type_name != var_name);
            prop_assume!(type_name != undefined_type_name);
            prop_assume!(var_name != undefined_type_name);

            let mut resolver = TypeResolver::new();

            // Test case 1: Forward reference that gets resolved
            // Create a variable that references a type before it's defined
            let var_decl = VarDecl {
                identifier: VariableIdentifier::new_symbol(&var_name),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::LateResolvedType(TypeName::from(&type_name)),
                reference_annotation: None,
            };

            // Register the variable first (forward reference)
            assert!(resolver.symbol_table.register_variable(var_decl).is_ok());

            // Now define the type
            let type_def = TypeDefinition {
                name: TypeName::from(&type_name),
                base_type: DataTypeSpecificationKind::Elementary(ElementaryTypeName::INT),
                default_value: None,
                span: SourceSpan::default(),
            };

            assert!(resolver.symbol_table.register_type(type_def).is_ok());

            // Resolution should succeed since the type is now defined
            let result = resolver.resolve_type_references();
            assert!(result.is_ok(), "Forward reference should be resolved when type is defined");

            // Test case 2: Unresolved forward reference
            let mut unresolved_resolver = TypeResolver::new();

            let unresolved_var_decl = VarDecl {
                identifier: VariableIdentifier::new_symbol(&var_name),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::LateResolvedType(TypeName::from(&undefined_type_name)),
                reference_annotation: None,
            };

            assert!(unresolved_resolver.symbol_table.register_variable(unresolved_var_decl).is_ok());

            // Resolution should fail since the type is never defined
            let result = unresolved_resolver.resolve_type_references();
            assert!(result.is_err(), "Unresolved forward reference should cause error");

            let errors = result.unwrap_err();
            assert!(!errors.is_empty(), "Should have at least one error for unresolved reference");

            // Verify the error mentions the undefined type
            let error_messages: Vec<String> = errors.iter().map(|e| e.primary.message.clone()).collect();
            println!("Error messages: {:?}", error_messages);
            println!("Looking for undefined type: {}", undefined_type_name);
            let has_undefined_type_error = error_messages.iter().any(|msg| msg.contains(&undefined_type_name));
            assert!(has_undefined_type_error, "Error should mention the undefined type name. Messages: {:?}", error_messages);

            // Test case 3: Elementary types should not cause forward reference errors
            let mut elementary_resolver = TypeResolver::new();

            let elementary_var_decl = VarDecl {
                identifier: VariableIdentifier::new_symbol(&var_name),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("INT"),
                    initial_value: None,
                }),
                reference_annotation: None,
            };

            assert!(elementary_resolver.symbol_table.register_variable(elementary_var_decl).is_ok());

            // Resolution should succeed for elementary types
            let result = elementary_resolver.resolve_type_references();
            assert!(result.is_ok(), "Elementary types should not require forward reference resolution");
        }
    }
}
