//! Transform that builds the symbol table and function environment.
//!
//! This transform populates:
//! - `SymbolEnvironment`: tracks declarations and scoping (variables, parameters, types, POUs)
//! - `FunctionEnvironment`: tracks function signatures for call validation
//!
//! Function signatures store type names (not resolved types) to allow building
//! complete signatures even when type resolution fails. Types are resolved
//! on-demand during validation via TypeEnvironment.
//!
//! A repeated declaration name is diagnosed here, by the environments: a
//! function repeating a function is `P4016` from the function environment, a
//! type repeating a type is `P2007` and any other pair of global declarations
//! is `P4013` from the symbol environment. A function and a symbol of one
//! name live in different environments, so this transform checks that pair
//! itself. The first declaration is kept and analysis continues on it; the
//! diagnostics are collected rather than aborting the walk.

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
use ironplc_problems::Problem;
use log::debug;
use std::convert::Infallible;

use crate::{
    function_environment::{FunctionEnvironment, FunctionSignature},
    intermediate_type::IntermediateFunctionParameter,
    symbol_environment::{
        duplicate_declaration, ScopeKind, ScopePath, SymbolEnvironment, SymbolKind,
    },
};

/// Populates the environments from `lib`. Always keeps the library: the
/// diagnostics are the repeated declaration names, and analysis continues
/// on the first declaration of each.
pub fn apply(
    lib: Library,
    symbol_environment: &mut SymbolEnvironment,
    function_environment: &mut FunctionEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let diagnostics = apply_impl(&lib, symbol_environment, function_environment);
    Ok((lib, diagnostics))
}

pub fn apply_impl(
    lib: &Library,
    symbol_env: &mut SymbolEnvironment,
    function_env: &mut FunctionEnvironment,
) -> Vec<Diagnostic> {
    let mut resolver = EnvironmentResolver {
        symbol_env,
        function_env,
        scope: Vec::new(),
        diagnostics: Vec::new(),
    };
    let Ok(()) = resolver.walk(lib);

    debug!("{:?}", resolver.symbol_env);

    resolver.diagnostics
}

struct EnvironmentResolver<'a> {
    symbol_env: &'a mut SymbolEnvironment,
    function_env: &'a mut FunctionEnvironment,
    /// The chain of declarations the traversal is currently inside,
    /// outermost first. A stack rather than a single name because
    /// declarations nest: a method is inside its function block.
    scope: Vec<Id>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> EnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        if self.scope.is_empty() {
            ScopeKind::Global
        } else {
            ScopeKind::Named(ScopePath::new(self.scope.clone()))
        }
    }

    /// Keeps the diagnostic an environment returned for a repeated name.
    fn record(&mut self, result: Result<(), Diagnostic>) {
        if let Err(diagnostic) = result {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Declares `name` in the global scope. A function already holds the
    /// name in the other environment, so that pair is checked here; every
    /// other repeat is the symbol environment's to report.
    fn declare_global(&mut self, name: &Id, kind: SymbolKind) {
        if let Some(function) = self.function_env.get(name) {
            self.diagnostics.push(duplicate_declaration(
                Problem::PouDeclNameDuplicated,
                name,
                function.span.clone(),
            ));
            return;
        }
        let result = self.symbol_env.insert(name, kind, &ScopeKind::Global);
        self.record(result);
    }
}

impl<'a> Visitor<Infallible> for EnvironmentResolver<'a> {
    type Value = ();

    /// Pushes the declaration the traversal is entering onto the scope
    /// stack, so the variables it declares are recorded against its own
    /// path rather than the enclosing declaration's.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
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
    ) -> Result<Self::Value, Infallible> {
        let symbol_kind = match node.var_type {
            VariableType::Input => SymbolKind::Parameter,
            VariableType::Output => SymbolKind::OutputParameter,
            VariableType::InOut => SymbolKind::InOutParameter,
            _ => SymbolKind::Variable,
        };

        match &node.identifier {
            ironplc_dsl::common::VariableIdentifier::Symbol(id) => {
                let result = self.symbol_env.insert_variable(
                    id,
                    symbol_kind,
                    &self.current_scope(),
                    node.var_type.clone(),
                    None,
                );
                self.record(result);
            }
            ironplc_dsl::common::VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    let address = format_address(&direct.address_assignment);
                    let result = self.symbol_env.insert_variable(
                        name,
                        symbol_kind,
                        &self.current_scope(),
                        node.var_type.clone(),
                        Some(address),
                    );
                    self.record(result);
                }
            }
        }
        node.recurse_visit(self)
    }

    fn visit_edge_var_decl(
        &mut self,
        node: &ironplc_dsl::common::EdgeVarDecl,
    ) -> Result<Self::Value, Infallible> {
        let result = self.symbol_env.insert(
            &node.identifier,
            SymbolKind::EdgeVariable,
            &self.current_scope(),
        );
        self.record(result);
        node.recurse_visit(self)
    }

    fn visit_function_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
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

        // A type, function block, program or configuration already holds
        // the name in the symbol environment: the function repeats it, and
        // that earlier declaration is kept. A function repeating a function
        // is the function environment's own P4016.
        if let Some(existing) = self.symbol_env.find(&node.name, &ScopeKind::Global) {
            if existing.scope == ScopeKind::Global
                && matches!(
                    existing.kind,
                    SymbolKind::Type
                        | SymbolKind::FunctionBlock
                        | SymbolKind::Program
                        | SymbolKind::Configuration
                )
            {
                self.diagnostics.push(duplicate_declaration(
                    Problem::PouDeclNameDuplicated,
                    &node.name,
                    existing.span.clone(),
                ));
                return node.recurse_visit(self);
            }
        }

        // Build and insert function signature
        let signature =
            FunctionSignature::new(node.name.clone(), return_type, parameters, node.name.span());
        let result = self.function_env.insert(signature);
        self.record(result);

        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name.name, SymbolKind::FunctionBlock);
        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ironplc_dsl::common::ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Program);
        node.recurse_visit(self)
    }

    fn visit_configuration_declaration(
        &mut self,
        node: &ironplc_dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Configuration);
        node.recurse_visit(self)
    }

    fn visit_interface_declaration(
        &mut self,
        node: &ironplc_dsl::common::InterfaceDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Type);
        node.recurse_visit(self)
    }

    fn visit_data_type_declaration_kind(
        &mut self,
        node: &ironplc_dsl::common::DataTypeDeclarationKind,
    ) -> Result<Self::Value, Infallible> {
        match node {
            ironplc_dsl::common::DataTypeDeclarationKind::Simple(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Structure(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Enumeration(_) => {
                // Declared by `visit_enumeration_declaration`, which the
                // recursion below reaches and which also records the values.
                // Declaring it here too would report the type as its own
                // repeat.
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Array(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Subrange(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::String(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::LateBound(_) => {
                // Skip late-bound types for now
            }
            ironplc_dsl::common::DataTypeDeclarationKind::StructureInitialization(_) => {
                // Skip structure initializations for now
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Reference(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_structure_element_declaration(
        &mut self,
        node: &ironplc_dsl::common::StructureElementDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let result = self.symbol_env.insert(
            &node.name,
            SymbolKind::StructureElement,
            &self.current_scope(),
        );
        self.record(result);
        node.recurse_visit(self)
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &ironplc_dsl::common::EnumerationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // Add the enumeration type itself
        self.declare_global(&node.type_name.name, SymbolKind::Type);

        // Add each enumeration value
        if let ironplc_dsl::common::SpecificationKind::Inline(values) = &node.spec_init.spec {
            for value in &values.values {
                let result = self.symbol_env.insert_enumeration_value(
                    &value.value,
                    &node.type_name,
                    &ScopeKind::Global,
                );
                self.record(result);
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
    use ironplc_problems::Problem;

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
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(diagnostics.is_empty());
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
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(diagnostics.is_empty());

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
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(diagnostics.is_empty());

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
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);

        assert!(diagnostics.is_empty());

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
        apply_impl(&library, &mut symbol_env, &mut function_env);
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

    // -----------------------------------------------------------------
    // Repeated declaration names, through the whole resolution pipeline so
    // the toposort's handling of a repeat is part of what is checked.
    // -----------------------------------------------------------------

    /// The problem codes analysis reports for `program`, in order.
    fn analyzed_codes(program: &str) -> Vec<String> {
        analyzed_codes_with(
            program,
            &ironplc_parser::options::CompilerOptions::default(),
        )
    }

    fn analyzed_codes_with(
        program: &str,
        options: &ironplc_parser::options::CompilerOptions,
    ) -> Vec<String> {
        let library =
            ironplc_parser::parse_program(program, &ironplc_dsl::core::FileId::default(), options)
                .unwrap();
        let (_library, context) = crate::stages::analyze(&[&library], options).unwrap();
        context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    #[test]
    fn apply_when_function_repeats_function_then_p4016() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION Foo : BOOL
  Foo := FALSE;
END_FUNCTION

FUNCTION Foo : BOOL
  Foo := TRUE;
END_FUNCTION"
            ),
            [Problem::FunctionDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_function_block_then_p4013() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION_BLOCK Bar
  VAR
    X : BOOL;
  END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Bar
  VAR
    Y : BOOL;
  END_VAR
END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_program_repeats_program_then_p4013() {
        assert_eq!(
            analyzed_codes(
                "
PROGRAM Baz
  VAR
    X : BOOL;
  END_VAR
END_PROGRAM

PROGRAM Baz
  VAR
    Y : BOOL;
  END_VAR
END_PROGRAM"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_configuration_repeats_configuration_then_p4013() {
        assert_eq!(
            analyzed_codes(
                "
PROGRAM Prg1
  VAR
    X : BOOL;
  END_VAR
END_PROGRAM

CONFIGURATION Cfg1
  RESOURCE Res1 ON PLC
    TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH Main : Prg1;
  END_RESOURCE
END_CONFIGURATION

CONFIGURATION Cfg1
  RESOURCE Res2 ON PLC
    TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P2 WITH Main : Prg1;
  END_RESOURCE
END_CONFIGURATION"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_type_repeats_type_then_p2007() {
        assert_eq!(
            analyzed_codes(
                "
TYPE
  the_struct : STRUCT
    member : BOOL;
  END_STRUCT;
  the_struct : STRUCT
    member : BOOL;
  END_STRUCT;
END_TYPE"
            ),
            [Problem::TypeDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_function_then_p4013() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION Compute : INT
  Compute := 0;
END_FUNCTION

FUNCTION_BLOCK Compute
  VAR
    X : INT;
  END_VAR
END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_repeats_function_block_then_p4013() {
        // The function block declares the name first in the sorted library,
        // so the function is the repeat and the block is kept.
        let program = "
FUNCTION_BLOCK Compute
  VAR
    X : INT;
  END_VAR
END_FUNCTION_BLOCK

FUNCTION Compute : INT
  Compute := 0;
END_FUNCTION";
        assert_eq!(
            analyzed_codes(program),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_block_repeats_type_then_p4013() {
        assert_eq!(
            analyzed_codes(
                "
TYPE
  Shared : INT := 0;
END_TYPE

FUNCTION_BLOCK Shared
  VAR
    X : INT;
  END_VAR
END_FUNCTION_BLOCK"
            ),
            [Problem::PouDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_names_differ_only_in_case_then_reported() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION foo : BOOL
  foo := FALSE;
END_FUNCTION

FUNCTION FOO : BOOL
  FOO := TRUE;
END_FUNCTION"
            ),
            [Problem::FunctionDeclNameDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_name_declared_three_times_then_reports_each_later_one() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION Foo : BOOL
  Foo := FALSE;
END_FUNCTION

FUNCTION Foo : BOOL
  Foo := TRUE;
END_FUNCTION

FUNCTION Foo : BOOL
  Foo := TRUE;
END_FUNCTION"
            )
            .len(),
            2
        );
    }

    /// The first declaration is the one kept, so the rest of analysis sees
    /// its signature.
    #[test]
    fn apply_when_function_repeated_then_first_signature_kept() {
        let program = "
FUNCTION Foo : BOOL
  VAR_INPUT
    a : INT;
  END_VAR
  Foo := FALSE;
END_FUNCTION

FUNCTION Foo : BOOL
  Foo := TRUE;
END_FUNCTION";
        let library = parse_and_resolve_types(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            function_env.get(&Id::from("Foo")).unwrap().parameters.len(),
            1
        );
    }

    // -----------------------------------------------------------------
    // Repeated variable names in one scope (P4014), through the whole
    // pipeline. Cross-scope hiding is ADR-0051's question and stays allowed.
    // -----------------------------------------------------------------

    const CONFIG: &str = "
CONFIGURATION config
  VAR_GLOBAL
    MaxSpeed : INT := 100;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
";

    fn with_config(pou: &str) -> String {
        format!("{CONFIG}{pou}")
    }

    fn top_level_globals() -> ironplc_parser::options::CompilerOptions {
        ironplc_parser::options::CompilerOptions {
            allow_top_level_var_global: true,
            ..ironplc_parser::options::CompilerOptions::default()
        }
    }

    fn top_level_globals_and_uptime() -> ironplc_parser::options::CompilerOptions {
        ironplc_parser::options::CompilerOptions {
            allow_system_uptime_global: true,
            ..top_level_globals()
        }
    }

    fn oop() -> ironplc_parser::options::CompilerOptions {
        ironplc_parser::options::CompilerOptions {
            allow_fb_inheritance: true,
            ..ironplc_parser::options::CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_variable_names_unique_across_blocks_then_no_diagnostics() {
        assert!(analyzed_codes(
            "
PROGRAM main
  VAR_INPUT
    start : BOOL;
  END_VAR
  VAR
    running : BOOL;
  END_VAR
END_PROGRAM"
        )
        .is_empty());
    }

    #[test]
    fn apply_when_variable_repeated_in_one_block_then_p4014() {
        assert_eq!(
            analyzed_codes(
                "
PROGRAM main
  VAR
    count : INT;
    second : INT;
    COUNT : BOOL;
  END_VAR
END_PROGRAM"
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_variable_repeated_across_blocks_then_p4014() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION_BLOCK fb
  VAR_INPUT
    start : BOOL;
  END_VAR
  VAR
    start : INT;
  END_VAR
END_FUNCTION_BLOCK"
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_input_repeated_as_output_then_p4014() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION f : INT
  VAR_INPUT
    Count : INT;
  END_VAR
  VAR_OUTPUT
    COUNT : INT;
  END_VAR
  f := Count;
END_FUNCTION"
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_variable_declared_three_times_then_reports_each_later_one() {
        assert_eq!(
            analyzed_codes(
                "
PROGRAM main
  VAR
    x : INT;
    x : INT;
    x : INT;
  END_VAR
END_PROGRAM"
            )
            .len(),
            2
        );
    }

    #[test]
    fn apply_when_edge_variable_named_like_variable_then_p4014() {
        assert_eq!(
            analyzed_codes(
                "
FUNCTION_BLOCK fb
  VAR_INPUT
    trigger : BOOL R_EDGE;
  END_VAR
  VAR
    trigger : INT;
  END_VAR
END_FUNCTION_BLOCK"
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_global_repeated_across_configuration_and_resource_then_p4014() {
        assert_eq!(
            analyzed_codes(
                "
CONFIGURATION config
  VAR_GLOBAL
    Limit : INT;
  END_VAR
  RESOURCE res ON PLC
    VAR_GLOBAL
      Limit : INT;
    END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
END_PROGRAM"
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_global_repeated_across_top_level_and_configuration_then_p4014() {
        assert_eq!(
            analyzed_codes_with(
                &with_config(
                    "
VAR_GLOBAL
  MaxSpeed : INT;
END_VAR

PROGRAM main
END_PROGRAM"
                ),
                &top_level_globals()
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_global_constant_repeated_then_p4014_once() {
        // The initializer-folding pass keeps its own table of constants and
        // must not report the repeat a second time.
        let options = ironplc_parser::options::CompilerOptions {
            allow_constant_initializer_expressions: true,
            ..top_level_globals()
        };
        assert_eq!(
            analyzed_codes_with(
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
END_PROGRAM",
                &options
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_program_names_global_through_var_external_then_no_diagnostics() {
        assert!(analyzed_codes(&with_config(
            "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
        ))
        .is_empty());
    }

    #[test]
    fn apply_when_var_external_declared_twice_then_p4014() {
        assert_eq!(
            analyzed_codes(&with_config(
                "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
            )),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_function_local_named_like_global_then_no_diagnostics() {
        assert!(analyzed_codes(&with_config(
            "
FUNCTION f : INT
  VAR
    MaxSpeed : INT := 5;
  END_VAR
  f := MaxSpeed;
END_FUNCTION

PROGRAM main
END_PROGRAM"
        ))
        .is_empty());
    }

    #[test]
    fn apply_when_method_local_named_like_function_block_field_then_no_diagnostics() {
        assert!(analyzed_codes_with(
            "
FUNCTION_BLOCK fb
  VAR
    speed : INT;
  END_VAR
  METHOD get : INT
    VAR
      speed : INT;
    END_VAR
    get := speed;
  END_METHOD
END_FUNCTION_BLOCK",
            &oop()
        )
        .is_empty());
    }

    #[test]
    fn apply_when_method_variable_repeated_then_p4014() {
        assert_eq!(
            analyzed_codes_with(
                "
FUNCTION_BLOCK fb
  METHOD get : INT
    VAR_INPUT
      speed : INT;
    END_VAR
    VAR
      speed : INT;
    END_VAR
    get := speed;
  END_METHOD
END_FUNCTION_BLOCK",
                &oop()
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_located_variables_have_no_name_then_no_diagnostics() {
        assert!(analyzed_codes(
            "
PROGRAM main
  VAR
    AT %IX0.0 : BOOL;
    AT %IX0.1 : BOOL;
  END_VAR
END_PROGRAM"
        )
        .is_empty());
    }

    /// Issue #1525: a global that redeclares a compiler-provided uptime global.
    #[test]
    fn apply_when_global_redeclares_system_uptime_with_flag_on_then_p4014() {
        assert_eq!(
            analyzed_codes_with(
                "
VAR_GLOBAL
  __SYSTEM_UP_TIME : TIME;
END_VAR

PROGRAM main
  VAR
    seen : TIME;
  END_VAR
  seen := __SYSTEM_UP_TIME;
END_PROGRAM",
                &top_level_globals_and_uptime()
            ),
            [Problem::SymbolDeclDuplicated.code()]
        );
    }

    #[test]
    fn apply_when_global_named_system_uptime_with_flag_off_then_no_diagnostics() {
        assert!(analyzed_codes_with(
            "
VAR_GLOBAL
  __SYSTEM_UP_TIME : TIME;
END_VAR

PROGRAM main
  VAR
    seen : TIME;
  END_VAR
  seen := __SYSTEM_UP_TIME;
END_PROGRAM",
            &top_level_globals()
        )
        .is_empty());
    }

    /// The primary label is on the later declaration and the secondary on
    /// the first, so the user sees both.
    #[test]
    fn apply_when_variable_repeated_then_labels_point_at_both_declarations() {
        let program = "
PROGRAM main
  VAR
    count : INT;
    COUNT : BOOL;
  END_VAR
END_PROGRAM";
        let library = parse_and_resolve_types(program);
        let mut symbol_env = SymbolEnvironment::new();
        let mut function_env = FunctionEnvironment::new();
        let diagnostics = apply_impl(&library, &mut symbol_env, &mut function_env);
        assert_eq!(diagnostics.len(), 1);
        let first = program.find("count").unwrap();
        let second = program.find("COUNT").unwrap();
        assert_eq!(diagnostics[0].primary.location.start, second);
        assert_eq!(diagnostics[0].secondary[0].location.start, first);
    }
}
