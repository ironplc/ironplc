//! Transform that builds the symbol table and function environment.
//!
//! This transform populates:
//! - `SymbolEnvironment`: tracks declarations and scoping (variables, parameters, types, POUs)
//! - `FunctionEnvironment`: tracks function signatures for call validation

use ironplc_dsl::{
    common::{Library, VariableType},
    core::{Id, Located},
    diagnostic::Diagnostic,
    visitor::Visitor,
};
use log::debug;

use crate::{
    function_environment::FunctionEnvironment,
    result::SemanticResult,
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: Library,
    _type_environment: &TypeEnvironment,
    symbol_environment: &mut SymbolEnvironment,
    _function_environment: &mut FunctionEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    apply_impl(&lib, symbol_environment)?;

    Ok(lib)
}

pub fn apply_impl(lib: &Library, env: &mut SymbolEnvironment) -> SemanticResult {
    let mut resolver = SymbolEnvironmentResolver { env, scope: None };
    let result = resolver.walk(lib).map_err(|e| vec![e]);

    debug!("{:?}", resolver.env);

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

struct SymbolEnvironmentResolver<'a> {
    env: &'a mut SymbolEnvironment,
    scope: Option<Id>,
}

impl<'a> SymbolEnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        match &self.scope {
            Some(name) => ScopeKind::Named(name.clone()),
            None => ScopeKind::Global,
        }
    }
}

impl<'a> Visitor<Diagnostic> for SymbolEnvironmentResolver<'a> {
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

                    self.env.insert(id, symbol_kind, &self.current_scope())?;
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
        self.env.insert(
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

        self.env
            .insert(&node.name, SymbolKind::Function, &ScopeKind::Global)?;

        let result = node.recurse_visit(self);
        self.scope = None;

        result
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.name.clone());
        self.env.insert(
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
        self.env
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
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Structure(decl) => {
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Enumeration(decl) => {
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Array(decl) => {
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Subrange(decl) => {
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
            }
            ironplc_dsl::common::DataTypeDeclarationKind::String(decl) => {
                self.env
                    .insert(&decl.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;
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
        self.env.insert(
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
        self.env
            .insert(&node.type_name.name, SymbolKind::Type, &ScopeKind::Global)?;

        // Add each enumeration value
        if let ironplc_dsl::common::SpecificationKind::Inline(values) = &node.spec_init.spec {
            for value in &values.values {
                self.env.insert_enumeration_value(
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
        symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind},
        test_helpers::parse_and_resolve_types,
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

        let library = parse_and_resolve_types(program);
        let mut env = SymbolEnvironment::new();
        let result = apply_impl(&library, &mut env);

        assert!(result.is_ok());
        let attributes = env
            .get(&Id::from("LEVEL"), &ScopeKind::Named(Id::from("LOGGER")))
            .unwrap();
        assert_eq!(attributes.kind, SymbolKind::Parameter);

        let attributes = env.get(&Id::from("LOGGER"), &ScopeKind::Global).unwrap();
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

        let library = parse_and_resolve_types(program);
        let mut env = SymbolEnvironment::new();
        let result = apply_impl(&library, &mut env);

        assert!(result.is_ok());

        // Check that input parameters are captured
        let reset_symbol = env
            .get(&Id::from("Reset"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(reset_symbol.kind, SymbolKind::Parameter);

        let count_symbol = env
            .get(&Id::from("Count"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(count_symbol.kind, SymbolKind::Parameter);

        // Check that output parameters are captured
        let out_symbol = env
            .get(&Id::from("OUT"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(out_symbol.kind, SymbolKind::OutputParameter);

        // Check that local variables are captured
        let cnt_symbol = env
            .get(&Id::from("Cnt"), &ScopeKind::Named(Id::from("Counter")))
            .unwrap();
        assert_eq!(cnt_symbol.kind, SymbolKind::Variable);

        // Check that function block is captured
        let counter_symbol = env.get(&Id::from("Counter"), &ScopeKind::Global).unwrap();
        assert_eq!(counter_symbol.kind, SymbolKind::FunctionBlock);
    }
}
