//! Transform that builds the symbol table and function environment.
//!
//! This transform populates:
//! - `SymbolEnvironment`: tracks declarations and scoping (variables, parameters, types, POUs)
//! - `FunctionEnvironment`: tracks function signatures for call validation

use ironplc_dsl::{
    common::{Library, TypeReference, VariableType},
    core::{Id, Located},
    diagnostic::Diagnostic,
    visitor::Visitor,
};
use log::debug;

use crate::{
    function_environment::{FunctionEnvironment, FunctionSignature},
    intermediate_type::IntermediateFunctionParameter,
    result::SemanticResult,
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &mut SymbolEnvironment,
    function_environment: &mut FunctionEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    apply_impl(
        &lib,
        type_environment,
        symbol_environment,
        function_environment,
    )?;

    Ok(lib)
}

pub fn apply_impl(
    lib: &Library,
    type_env: &TypeEnvironment,
    symbol_env: &mut SymbolEnvironment,
    function_env: &mut FunctionEnvironment,
) -> SemanticResult {
    let mut resolver = EnvironmentResolver {
        symbol_env,
        function_env,
        type_env,
        scope: None,
    };
    let result = resolver.walk(lib).map_err(|e| vec![e]);

    debug!("{:?}", resolver.symbol_env);

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

struct EnvironmentResolver<'a> {
    symbol_env: &'a mut SymbolEnvironment,
    function_env: &'a mut FunctionEnvironment,
    type_env: &'a TypeEnvironment,
    scope: Option<Id>,
}

impl<'a> EnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        match &self.scope {
            Some(name) => ScopeKind::Named(name.clone()),
            None => ScopeKind::Global,
        }
    }
}

impl<'a> Visitor<Diagnostic> for EnvironmentResolver<'a> {
    type Value = ();

    // TODO fn visit_program_access_decl

    #[allow(unused_assignments)]
    fn visit_var_decl(
        &mut self,
        node: &ironplc_dsl::common::VarDecl,
    ) -> Result<Self::Value, Diagnostic> {
        // Some types of variables are references to other variables.
        // TODO Stage these so that we can update references to them but not actually declare them here
        // (or otherwise distinguish between a declaration and a reference)
        if node.var_type != VariableType::External {
            match &node.identifier {
                ironplc_dsl::common::VariableIdentifier::Symbol(id) => {
                    // Determine the appropriate symbol kind based on variable type
                    let symbol_kind = match node.var_type {
                        VariableType::Input => SymbolKind::Parameter,
                        VariableType::Output => SymbolKind::OutputParameter,
                        VariableType::InOut => SymbolKind::InOutParameter,
                        VariableType::Var | VariableType::VarTemp => SymbolKind::Variable,
                        VariableType::Global => SymbolKind::Variable, // Global variables
                        VariableType::Access => SymbolKind::Variable, // Access variables
                        VariableType::External => SymbolKind::Variable, // Should not reach here due to above check
                    };

                    self.symbol_env
                        .insert(id, symbol_kind, &self.current_scope())?;
                }
                ironplc_dsl::common::VariableIdentifier::Direct(_) => {
                    // TODO: Handle direct variables (hardware-mapped I/O)
                }
            }
        } else {
            // External variables are references to global variables
            if let ironplc_dsl::common::VariableIdentifier::Symbol(id) = &node.identifier {
                // Mark as external reference
                let mut symbol_info = crate::symbol_environment::SymbolInfo::new(
                    SymbolKind::Variable,
                    self.current_scope(),
                    id.span(),
                );
                symbol_info = symbol_info.with_external(true);
                // TODO: Need to modify insert to handle external references
            }
        }
        node.recurse_visit(self)
    }

    fn visit_edge_var_decl(
        &mut self,
        node: &ironplc_dsl::common::EdgeVarDecl,
    ) -> Result<Self::Value, Diagnostic> {
        self.symbol_env.insert(
            &node.identifier,
            SymbolKind::EdgeVariable,
            &self.current_scope(),
        )?;
        node.recurse_visit(self)
    }

    fn visit_function_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.clone());

        // Register function in symbol environment
        self.symbol_env
            .insert(&node.name, SymbolKind::Function, &ScopeKind::Global)?;

        // Build function signature for function environment
        // Collect parameters (INPUT, OUTPUT, INOUT variables)
        let mut parameters = Vec::new();
        for var_decl in &node.variables {
            let is_parameter = matches!(
                var_decl.var_type,
                VariableType::Input | VariableType::Output | VariableType::InOut
            );
            if !is_parameter {
                continue;
            }

            // Get parameter name
            let param_name = match &var_decl.identifier {
                ironplc_dsl::common::VariableIdentifier::Symbol(id) => id.clone(),
                ironplc_dsl::common::VariableIdentifier::Direct(_) => continue,
            };

            // Get parameter type
            let param_type = match var_decl.type_name() {
                TypeReference::Named(type_name) => {
                    match self.type_env.get(&type_name) {
                        Some(attrs) => attrs.representation.clone(),
                        None => continue, // Type not found, skip this parameter
                    }
                }
                _ => continue, // Inline or unspecified types not supported yet
            };

            parameters.push(IntermediateFunctionParameter {
                name: param_name,
                param_type,
                is_input: var_decl.var_type == VariableType::Input,
                is_output: var_decl.var_type == VariableType::Output,
                is_inout: var_decl.var_type == VariableType::InOut,
            });
        }

        // Get return type
        let return_type = self
            .type_env
            .get(&node.return_type)
            .map(|attrs| attrs.representation.clone());

        // Build and insert function signature
        let signature =
            FunctionSignature::new(node.name.clone(), return_type, parameters, node.name.span());
        self.function_env.insert(signature)?;

        let result = node.recurse_visit(self);
        self.scope = None;

        result
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.name.clone());
        self.symbol_env.insert(
            &node.name.name,
            SymbolKind::FunctionBlock,
            &ScopeKind::Global,
        )?;
        let result = node.recurse_visit(self);
        self.scope = None;

        result
    }

    fn visit_program_declaration(
        &mut self,
        node: &ironplc_dsl::common::ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.clone());
        self.symbol_env
            .insert(&node.name, SymbolKind::Program, &ScopeKind::Global)?;
        let result = node.recurse_visit(self);
        self.scope = None;

        result
    }

    fn visit_data_type_declaration_kind(
        &mut self,
        node: &ironplc_dsl::common::DataTypeDeclarationKind,
    ) -> Result<Self::Value, Diagnostic> {
        match node {
            ironplc_dsl::common::DataTypeDeclarationKind::Simple(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Structure(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Enumeration(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Array(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Subrange(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::String(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::LateBound(_) => {
                // Skip late-bound types for now
            }
            ironplc_dsl::common::DataTypeDeclarationKind::StructureInitialization(_) => {
                // Skip structure initializations for now
            }
        }
        node.recurse_visit(self)
    }

    fn visit_structure_element_declaration(
        &mut self,
        node: &ironplc_dsl::common::StructureElementDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.symbol_env.insert(
            &node.name,
            SymbolKind::StructureElement,
            &self.current_scope(),
        )?;
        node.recurse_visit(self)
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &ironplc_dsl::common::EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        // Add the enumeration type itself
        self.symbol_env
            .insert(&node.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;

        // Add each enumeration value
        if let ironplc_dsl::common::SpecificationKind::Inline(values) = &node.spec_init.spec {
            for value in &values.values {
                self.symbol_env.insert_enumeration_value(
                    &value.value,
                    &node.type_name,
                    &ScopeKind::Global,
                )?;
            }
        }

        node.recurse_visit(self)
    }

    // TODO should this handle parameters?
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::Id;

    use crate::{
        function_environment::FunctionEnvironment,
        intermediate_type::{ByteSized, IntermediateType},
        symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
        test_helpers::parse_and_resolve_types_with_type_env,
        xform_resolve_symbol_and_function_environment::apply_impl,
    };

    #[test]
    fn apply_when_var_init_valid_enum_value_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let (library, type_env) = parse_and_resolve_types_with_type_env(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &type_env, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());
        let attributes = symbol_env
            .get(&Id::from("LEVEL"), &ScopeKind::Named(Id::from("LOGGER")))
            .unwrap();
        assert_eq!(attributes.kind, SymbolKind::Parameter);

        let attributes = symbol_env
            .get(&Id::from("LOGGER"), &ScopeKind::Global)
            .unwrap();
        assert_eq!(attributes.kind, SymbolKind::FunctionBlock);
    }

    #[test]
    fn apply_when_function_block_has_parameters_then_parameters_are_symbols() {
        let program = "
FUNCTION_BLOCK Counter
VAR_INPUT
    Reset : BOOL;
    Count : INT;
END_VAR
VAR_OUTPUT
    OUT : INT;
END_VAR
VAR
    Cnt : INT;
END_VAR
END_FUNCTION_BLOCK";

        let (library, type_env) = parse_and_resolve_types_with_type_env(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &type_env, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        // Check that input parameters are captured
        let reset_symbol = symbol_env
            .get(&Id::from("Reset"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(reset_symbol.kind, SymbolKind::Parameter);

        let count_symbol = symbol_env
            .get(&Id::from("Count"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(count_symbol.kind, SymbolKind::Parameter);

        // Check that output parameters are captured
        let out_symbol = symbol_env
            .get(&Id::from("OUT"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(out_symbol.kind, SymbolKind::OutputParameter);

        // Check that local variables are captured
        let cnt_symbol = symbol_env
            .get(&Id::from("Cnt"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(cnt_symbol.kind, SymbolKind::Variable);

        // Check that function block is captured
        let counter_symbol = symbol_env
            .get(&Id::from("Counter"), &ScopeKind::Global)
            .unwrap();
        assert_eq!(counter_symbol.kind, SymbolKind::FunctionBlock);
    }

    #[test]
    fn apply_when_function_declaration_then_populates_function_environment() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION";

        let (library, type_env) = parse_and_resolve_types_with_type_env(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &type_env, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        // Check function is in symbol environment
        let func_symbol = symbol_env
            .get(&Id::from("ADD_INTS"), &ScopeKind::Global)
            .unwrap();
        assert_eq!(func_symbol.kind, SymbolKind::Function);

        // Check function is in function environment with correct signature
        let func_sig = function_env.get(&Id::from("ADD_INTS")).unwrap();
        assert_eq!(func_sig.name.original(), "ADD_INTS");
        assert_eq!(
            func_sig.return_type,
            Some(IntermediateType::Int {
                size: ByteSized::B16
            })
        );
        assert_eq!(func_sig.parameters.len(), 2);

        // Check first parameter
        assert_eq!(func_sig.parameters[0].name.original(), "A");
        assert!(func_sig.parameters[0].is_input);
        assert!(!func_sig.parameters[0].is_output);

        // Check second parameter
        assert_eq!(func_sig.parameters[1].name.original(), "B");
        assert!(func_sig.parameters[1].is_input);
    }

    #[test]
    fn apply_when_function_with_output_param_then_captures_output() {
        let program = "
FUNCTION SPLIT : INT
VAR_INPUT
    Value : INT;
END_VAR
VAR_OUTPUT
    High : INT;
    Low : INT;
END_VAR
    High := Value / 256;
    Low := Value MOD 256;
    SPLIT := 0;
END_FUNCTION";

        let (library, type_env) = parse_and_resolve_types_with_type_env(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &type_env, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        let func_sig = function_env.get(&Id::from("SPLIT")).unwrap();
        assert_eq!(func_sig.parameters.len(), 3);

        // Check input parameter
        assert!(func_sig.parameters[0].is_input);

        // Check output parameters
        assert!(func_sig.parameters[1].is_output);
        assert!(func_sig.parameters[2].is_output);
    }
}
