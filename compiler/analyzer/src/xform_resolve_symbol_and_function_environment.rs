//! Transform that builds the symbol table and function environment.
//!
//! This transform populates:
//! - `SymbolEnvironment`: tracks declarations and scoping (variables, parameters, types, POUs)
//! - `FunctionEnvironment`: tracks function signatures for call validation
//!
//! Function signatures store type names (not resolved types) to allow building
//! complete signatures even when type resolution fails. Types are resolved
//! on-demand during validation via TypeEnvironment.

use ironplc_dsl::{
    common::{
        AddressAssignment, InitialValueAssignmentKind, Library, LocationPrefix, SizePrefix,
        TypeReference, VariableType,
    },
    core::{Id, Located},
    diagnostic::Diagnostic,
    scope::ScopeNode,
    visitor::Visitor,
};
use log::debug;

use crate::{
    function_environment::{FunctionEnvironment, FunctionSignature},
    intermediate_type::IntermediateFunctionParameter,
    result::SemanticResult,
    symbol_environment::{ScopeKind, ScopePath, SymbolEnvironment, SymbolKind},
};

pub fn apply(
    lib: Library,
    symbol_environment: &mut SymbolEnvironment,
    function_environment: &mut FunctionEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    apply_impl(&lib, symbol_environment, function_environment)?;

    Ok(lib)
}

pub fn apply_impl(
    lib: &Library,
    symbol_env: &mut SymbolEnvironment,
    function_env: &mut FunctionEnvironment,
) -> SemanticResult {
    let mut resolver = EnvironmentResolver {
        symbol_env,
        function_env,
        scope: Vec::new(),
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
    /// The chain of declarations the traversal is currently inside,
    /// outermost first. A stack rather than a single name because
    /// declarations nest: a method is inside its function block.
    scope: Vec<Id>,
}

impl<'a> EnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        if self.scope.is_empty() {
            ScopeKind::Global
        } else {
            ScopeKind::Named(ScopePath::new(self.scope.clone()))
        }
    }
}

impl<'a> Visitor<Diagnostic> for EnvironmentResolver<'a> {
    type Value = ();

    /// Pushes the declaration the traversal is entering onto the scope
    /// stack, so the variables it declares are recorded against its own
    /// path rather than the enclosing declaration's.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.scope.push(match node {
            ScopeNode::Function(node) => node.name.clone(),
            ScopeNode::FunctionBlock(node) => node.name.name.clone(),
            ScopeNode::Program(node) => node.name.clone(),
            ScopeNode::Method(node) => node.name.clone(),
        });
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.pop();
    }

    // TODO fn visit_program_access_decl

    fn visit_var_decl(
        &mut self,
        node: &ironplc_dsl::common::VarDecl,
    ) -> Result<Self::Value, Diagnostic> {
        let symbol_kind = match node.var_type {
            VariableType::Input => SymbolKind::Parameter,
            VariableType::Output => SymbolKind::OutputParameter,
            VariableType::InOut => SymbolKind::InOutParameter,
            _ => SymbolKind::Variable,
        };

        match &node.identifier {
            ironplc_dsl::common::VariableIdentifier::Symbol(id) => {
                self.symbol_env.insert_variable(
                    id,
                    symbol_kind,
                    &self.current_scope(),
                    node.var_type.clone(),
                    None,
                )?;
            }
            ironplc_dsl::common::VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    let address = format_address(&direct.address_assignment);
                    self.symbol_env.insert_variable(
                        name,
                        symbol_kind,
                        &self.current_scope(),
                        node.var_type.clone(),
                        Some(address),
                    )?;
                }
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
        // Build function signature for function environment
        // (Functions are tracked in FunctionEnvironment, not SymbolEnvironment)
        // Collect parameters (INPUT, OUTPUT, INOUT variables)
        //
        // Note: We store TypeName references, not resolved types. This allows
        // building complete signatures even when type resolution fails. Types
        // are resolved on-demand during validation via TypeEnvironment.
        let mut parameters = Vec::new();
        for var_decl in &node.variables {
            if !var_decl.var_type.is_parameter() {
                continue;
            }

            // Get parameter name
            let param_name = match &var_decl.identifier {
                ironplc_dsl::common::VariableIdentifier::Symbol(id) => id.clone(),
                ironplc_dsl::common::VariableIdentifier::Direct(_) => continue,
            };

            // Get parameter type name (store as TypeName, resolve later).
            // REF_TO parameters report TypeReference::Inline from type_name(),
            // so we check the initializer directly to extract the referenced type.
            let (param_type, is_reference) = match var_decl.type_name() {
                TypeReference::Named(type_name) => (type_name, false),
                TypeReference::Inline => match &var_decl.initializer {
                    InitialValueAssignmentKind::Reference(ref_init) => {
                        match &ref_init.target {
                            crate::ironplc_dsl::common::ReferenceTarget::Named(tn) => {
                                (tn.clone(), true)
                            }
                            crate::ironplc_dsl::common::ReferenceTarget::Array(subranges) => {
                                // REF_TO ARRAY[...] OF T — use the element type name
                                // so the parameter is registered in the function signature.
                                (subranges.type_name.to_type_name(), true)
                            }
                        }
                    }
                    _ => continue,
                },
                _ => continue,
            };

            parameters.push(IntermediateFunctionParameter {
                name: param_name,
                param_type,
                is_input: var_decl.var_type == VariableType::Input,
                is_output: var_decl.var_type == VariableType::Output,
                is_inout: var_decl.var_type == VariableType::InOut,
                is_reference,
            });
        }

        // Store return type as TypeName (resolve later during validation)
        let return_type = Some(node.return_type.clone());

        // Build and insert function signature
        let signature =
            FunctionSignature::new(node.name.clone(), return_type, parameters, node.name.span());
        self.function_env.insert(signature)?;

        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.symbol_env.insert(
            &node.name.name,
            SymbolKind::FunctionBlock,
            &ScopeKind::Global,
        )?;
        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ironplc_dsl::common::ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.symbol_env
            .insert(&node.name, SymbolKind::Program, &ScopeKind::Global)?;
        node.recurse_visit(self)
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
            ironplc_dsl::common::DataTypeDeclarationKind::Reference(decl) => {
                self.symbol_env.insert(
                    &decl.type_name.name,
                    SymbolKind::Type,
                    &ScopeKind::Global,
                )?;
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

fn format_address(addr: &AddressAssignment) -> String {
    let loc = match addr.location {
        LocationPrefix::I => "I",
        LocationPrefix::Q => "Q",
        LocationPrefix::M => "M",
    };
    let size = match addr.size {
        SizePrefix::X => "X",
        SizePrefix::B => "B",
        SizePrefix::W => "W",
        SizePrefix::D => "D",
        SizePrefix::L => "L",
        SizePrefix::Nil | SizePrefix::Unspecified => "",
    };
    let parts: Vec<String> = addr.address.iter().map(|a| a.to_string()).collect();
    format!("%{loc}{size}{}", parts.join("."))
}

#[cfg(test)]
mod test {
    use ironplc_dsl::common::{FunctionReturnType, TypeName};
    use ironplc_dsl::core::Id;

    use crate::{
        function_environment::FunctionEnvironment,
        symbol_environment::{ScopeKind, ScopePath, SymbolEnvironment, SymbolKind},
        test_helpers::{parse_and_resolve_types, parse_and_resolve_types_with_options},
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
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());
        let attributes = symbol_env
            .get(
                &Id::from("LEVEL"),
                &ScopeKind::Named(Id::from("LOGGER").into()),
            )
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

        let library = parse_and_resolve_types(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        // Check that input parameters are captured
        let reset_symbol = symbol_env
            .get(
                &Id::from("Reset"),
                &ScopeKind::Named(Id::from("Counter").into()),
            )
            .unwrap();
        assert_eq!(reset_symbol.kind, SymbolKind::Parameter);

        let count_symbol = symbol_env
            .get(
                &Id::from("Count"),
                &ScopeKind::Named(Id::from("Counter").into()),
            )
            .unwrap();
        assert_eq!(count_symbol.kind, SymbolKind::Parameter);

        // Check that output parameters are captured
        let out_symbol = symbol_env
            .get(
                &Id::from("OUT"),
                &ScopeKind::Named(Id::from("Counter").into()),
            )
            .unwrap();
        assert_eq!(out_symbol.kind, SymbolKind::OutputParameter);

        // Check that local variables are captured
        let cnt_symbol = symbol_env
            .get(
                &Id::from("Cnt"),
                &ScopeKind::Named(Id::from("Counter").into()),
            )
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

        let library = parse_and_resolve_types(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        // Functions are NOT registered in symbol environment (only in function environment)
        assert!(symbol_env
            .get(&Id::from("ADD_INTS"), &ScopeKind::Global)
            .is_none());

        // Check function is in function environment with correct signature
        let func_sig = function_env.get(&Id::from("ADD_INTS")).unwrap();
        assert_eq!(func_sig.name.original(), "ADD_INTS");
        // Return type is now stored as TypeName, not resolved IntermediateType
        assert_eq!(
            func_sig.return_type,
            Some(FunctionReturnType::Named(TypeName::from("INT")))
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

        let library = parse_and_resolve_types(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let result = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(result.is_ok());

        let func_sig = function_env.get(&Id::from("SPLIT")).unwrap();
        assert_eq!(func_sig.parameters.len(), 3);

        // Check input parameter
        assert!(func_sig.parameters[0].is_input);

        // Check output parameters
        assert!(func_sig.parameters[1].is_output);
        assert!(func_sig.parameters[2].is_output);
    }

    // ---------------------------------------------------------------------
    // METHOD scoping.
    // See https://github.com/ironplc/ironplc/issues/1439.
    // ---------------------------------------------------------------------

    fn resolve_with_methods(program: &str) -> (SymbolEnvironment, FunctionEnvironment) {
        let options = ironplc_parser::options::CompilerOptions {
            allow_fb_inheritance: true,
            ..ironplc_parser::options::CompilerOptions::default()
        };
        let (library, _context) = parse_and_resolve_types_with_options(program, &options);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        apply_impl(&library, &mut symbol_env, &mut function_env).unwrap();
        (symbol_env, function_env)
    }

    fn method_scope(function_block: &str, method: &str) -> ScopeKind {
        ScopeKind::Named(ScopePath::new(vec![
            Id::from(function_block),
            Id::from(method),
        ]))
    }

    /// A method's parameters belong to the method. They used to be
    /// recorded against the enclosing function block, which is what made
    /// them visible to its siblings.
    #[test]
    fn apply_when_method_has_parameter_then_recorded_in_method_scope() {
        let (symbol_env, _) = resolve_with_methods(
            "
FUNCTION_BLOCK FB_Motor
VAR
    speed : INT;
END_VAR
METHOD SetSpeed
VAR_INPUT
    newSpeed : INT;
END_VAR
    speed := newSpeed;
END_METHOD
END_FUNCTION_BLOCK",
        );

        let param = Id::from("newSpeed");
        let fb_scope = ScopeKind::Named(Id::from("FB_Motor").into());

        assert!(
            symbol_env
                .get_variables_in_scope(&method_scope("FB_Motor", "SetSpeed"))
                .iter()
                .any(|(name, _)| *name == &param),
            "the parameter belongs to the method's own scope"
        );
        assert!(
            !symbol_env
                .get_variables_in_scope(&fb_scope)
                .iter()
                .any(|(name, _)| *name == &param),
            "and not to the function block's"
        );
    }

    /// Sibling methods are sibling scopes, so the same name in each is
    /// two distinct symbols rather than one overwriting the other.
    #[test]
    fn apply_when_two_methods_declare_same_name_then_each_scope_has_its_own() {
        let (symbol_env, _) = resolve_with_methods(
            "
FUNCTION_BLOCK FB_Motor
METHOD a
VAR
    q : INT;
END_VAR
    q := 1;
END_METHOD
METHOD b
VAR
    q : INT;
END_VAR
    q := 2;
END_METHOD
END_FUNCTION_BLOCK",
        );

        for method in ["a", "b"] {
            assert!(
                symbol_env
                    .get_variables_in_scope(&method_scope("FB_Motor", method))
                    .iter()
                    .any(|(name, _)| *name == &Id::from("q")),
                "method {method} should have its own q"
            );
        }
    }

    /// The method scope nests inside the function block's, so a lookup
    /// from inside a method still reaches the instance's fields.
    #[test]
    fn apply_when_looking_up_field_from_method_scope_then_found() {
        let (symbol_env, _) = resolve_with_methods(
            "
FUNCTION_BLOCK FB_Motor
VAR
    speed : INT;
END_VAR
METHOD SetSpeed
VAR_INPUT
    newSpeed : INT;
END_VAR
    speed := newSpeed;
END_METHOD
END_FUNCTION_BLOCK",
        );

        assert!(symbol_env
            .find(&Id::from("speed"), &method_scope("FB_Motor", "SetSpeed"))
            .is_some());
    }
}
