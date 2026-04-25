//! Neutral views and extractors over a `SemanticContext`.
//!
//! Multiple front-ends (the LSP server, the MCP server, future tooling)
//! need to enumerate programs, function blocks, functions, types, and
//! variables out of a completed semantic analysis. Without a shared
//! traversal, each front-end re-implements the iterate → filter → project
//! pattern and applies its own filter predicates, which has produced
//! divergent behavior in the past (see plan
//! `specs/plans/2026-04-25-shared-symbol-extractors.md`).
//!
//! This module owns the traversal. It returns borrow-based views that
//! preserve the underlying analyzer data, so callers map cheaply to their
//! protocol-specific shape (LSP `DocumentSymbol`, MCP JSON, etc.) without
//! re-traversing.
//!
//! No filtering by source file is performed here — that is a concern of
//! callers that scope output to a single document.

use ironplc_dsl::common::{TypeName, VariableType};
use ironplc_dsl::core::Id;

use crate::function_environment::FunctionSignature;
use crate::intermediate_type::IntermediateType;
use crate::semantic_context::SemanticContext;
use crate::symbol_environment::{ScopeKind, SymbolEnvironment, SymbolInfo, SymbolKind};
use crate::type_attributes::TypeAttributes;

/// Direction of a variable as seen by an outline / symbol view.
///
/// Mirrors the strings emitted by the MCP `symbols` tool. LSP currently
/// does not surface variables in the outline but can use the same
/// classification when it adds nested children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableDirection {
    In,
    Out,
    InOut,
    Local,
    Global,
    External,
}

impl VariableDirection {
    /// Stable string form, matching the MCP `symbols` JSON contract.
    pub fn as_str(self) -> &'static str {
        match self {
            VariableDirection::In => "In",
            VariableDirection::Out => "Out",
            VariableDirection::InOut => "InOut",
            VariableDirection::Local => "Local",
            VariableDirection::Global => "Global",
            VariableDirection::External => "External",
        }
    }
}

/// Coarse classification of a user-defined type, suitable for outline
/// rendering. Maps cleanly onto LSP's `SymbolKind` and the MCP `kind`
/// string field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSymbolKind {
    Enumeration,
    Structure,
    Array,
    Subrange,
    String,
    Reference,
    FunctionBlock,
    Function,
    Alias,
}

impl TypeSymbolKind {
    /// Stable string form, matching the MCP `types[].kind` JSON contract.
    pub fn as_str(self) -> &'static str {
        match self {
            TypeSymbolKind::Enumeration => "enumeration",
            TypeSymbolKind::Structure => "structure",
            TypeSymbolKind::Array => "array",
            TypeSymbolKind::Subrange => "subrange",
            TypeSymbolKind::String => "string",
            TypeSymbolKind::Reference => "reference",
            TypeSymbolKind::FunctionBlock => "function_block",
            TypeSymbolKind::Function => "function",
            TypeSymbolKind::Alias => "alias",
        }
    }
}

/// Classify an `IntermediateType` for outline display.
pub fn classify_type_kind(intermediate: &IntermediateType) -> TypeSymbolKind {
    match intermediate {
        IntermediateType::Enumeration { .. } => TypeSymbolKind::Enumeration,
        IntermediateType::Structure { .. } => TypeSymbolKind::Structure,
        IntermediateType::Array { .. } => TypeSymbolKind::Array,
        IntermediateType::Subrange { .. } => TypeSymbolKind::Subrange,
        IntermediateType::String { .. } => TypeSymbolKind::String,
        IntermediateType::Reference { .. } => TypeSymbolKind::Reference,
        IntermediateType::FunctionBlock { .. } => TypeSymbolKind::FunctionBlock,
        IntermediateType::Function { .. } => TypeSymbolKind::Function,
        _ => TypeSymbolKind::Alias,
    }
}

/// Determine the outline direction for a symbol from its declared
/// variable type and symbol kind, matching the rules used by the MCP
/// `symbols` tool.
pub fn normalize_variable_direction(info: &SymbolInfo) -> VariableDirection {
    match &info.variable_type {
        Some(VariableType::Input) => VariableDirection::In,
        Some(VariableType::Output) => VariableDirection::Out,
        Some(VariableType::InOut) => VariableDirection::InOut,
        Some(VariableType::Global) => VariableDirection::Global,
        Some(VariableType::External) => VariableDirection::External,
        _ => match info.kind {
            SymbolKind::Parameter => VariableDirection::In,
            SymbolKind::OutputParameter => VariableDirection::Out,
            SymbolKind::InOutParameter => VariableDirection::InOut,
            _ => VariableDirection::Local,
        },
    }
}

/// A view of a single variable in some scope.
#[derive(Debug, Clone, Copy)]
pub struct VariableSymbol<'a> {
    pub name: &'a Id,
    pub info: &'a SymbolInfo,
    pub direction: VariableDirection,
}

/// A view of a `PROGRAM` declaration, including its variables.
#[derive(Debug)]
pub struct ProgramSymbol<'a> {
    pub name: &'a Id,
    pub info: &'a SymbolInfo,
    pub variables: Vec<VariableSymbol<'a>>,
}

/// A view of a `FUNCTION_BLOCK` declaration, including its variables.
#[derive(Debug)]
pub struct FunctionBlockSymbol<'a> {
    pub name: &'a Id,
    pub info: &'a SymbolInfo,
    pub variables: Vec<VariableSymbol<'a>>,
}

/// A view of a user-defined `FUNCTION` declaration.
#[derive(Debug)]
pub struct FunctionSymbolView<'a> {
    pub signature: &'a FunctionSignature,
}

impl<'a> FunctionSymbolView<'a> {
    pub fn return_type_name(&self) -> Option<TypeName> {
        self.signature
            .return_type
            .as_ref()
            .map(|rt| rt.to_type_name())
    }

    /// Direction-classified parameters (input/output/inout) suitable for
    /// outline rendering.
    pub fn parameters(&self) -> impl Iterator<Item = ParameterView<'_>> {
        self.signature.parameters.iter().map(|p| {
            let direction = if p.is_inout {
                VariableDirection::InOut
            } else if p.is_output {
                VariableDirection::Out
            } else {
                VariableDirection::In
            };
            ParameterView {
                param: p,
                direction,
            }
        })
    }
}

/// A function parameter, augmented with its outline direction.
#[derive(Debug, Clone, Copy)]
pub struct ParameterView<'a> {
    pub param: &'a crate::intermediate_type::IntermediateFunctionParameter,
    pub direction: VariableDirection,
}

/// A view of a user-defined type declaration.
#[derive(Debug)]
pub struct TypeSymbolView<'a> {
    pub name: &'a TypeName,
    pub attributes: &'a TypeAttributes,
    pub kind: TypeSymbolKind,
}

/// All `PROGRAM` declarations in the global scope, with their variables
/// resolved.
pub fn extract_programs(context: &SemanticContext) -> Vec<ProgramSymbol<'_>> {
    context
        .symbols()
        .get_programs()
        .into_iter()
        .map(|(name, info)| {
            let scope = ScopeKind::Named(name.clone());
            let variables = extract_variables_in_scope(context.symbols(), &scope);
            ProgramSymbol {
                name,
                info,
                variables,
            }
        })
        .collect()
}

/// All `FUNCTION_BLOCK` declarations in the global scope, with their
/// variables resolved.
pub fn extract_function_blocks(context: &SemanticContext) -> Vec<FunctionBlockSymbol<'_>> {
    context
        .symbols()
        .get_function_blocks()
        .into_iter()
        .map(|(name, info)| {
            let scope = ScopeKind::Named(name.clone());
            let variables = extract_variables_in_scope(context.symbols(), &scope);
            FunctionBlockSymbol {
                name,
                info,
                variables,
            }
        })
        .collect()
}

/// User-defined (non-stdlib) `FUNCTION` declarations.
pub fn extract_user_defined_functions(context: &SemanticContext) -> Vec<FunctionSymbolView<'_>> {
    context
        .functions()
        .iter_user_defined()
        .map(|(_, signature)| FunctionSymbolView { signature })
        .collect()
}

/// User-defined types, excluding elementary types and excluding the
/// auto-generated entries for function blocks and functions (which are
/// surfaced separately by `extract_function_blocks` and
/// `extract_user_defined_functions`).
pub fn extract_user_defined_types(context: &SemanticContext) -> Vec<TypeSymbolView<'_>> {
    context
        .types()
        .iter_user_defined()
        .map(|(name, attributes)| TypeSymbolView {
            name,
            attributes,
            kind: classify_type_kind(&attributes.representation),
        })
        .collect()
}

/// Variables (locals, parameters) declared in the given scope, with
/// their outline direction normalized.
pub fn extract_variables_in_scope<'a>(
    symbols: &'a SymbolEnvironment,
    scope: &ScopeKind,
) -> Vec<VariableSymbol<'a>> {
    symbols
        .get_variables_in_scope(scope)
        .into_iter()
        .map(|(name, info)| VariableSymbol {
            name,
            info,
            direction: normalize_variable_direction(info),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::CompilerOptions;

    fn ed2_options() -> CompilerOptions {
        CompilerOptions::default()
    }

    fn analyze_source(source: &str) -> SemanticContext {
        let file_id = FileId::from_string("test.st");
        let library =
            ironplc_parser::parse_program(source, &file_id, &ed2_options()).expect("parse failed");
        let (_lib, ctx) = analyze(&[&library], &ed2_options()).expect("semantic analysis failed");
        ctx
    }

    #[test]
    fn classify_type_kind_when_enumeration_then_returns_enumeration() {
        let kind = classify_type_kind(&IntermediateType::Enumeration {
            underlying_type: Box::new(IntermediateType::Int {
                size: crate::intermediate_type::ByteSized::B8,
            }),
        });
        assert_eq!(kind, TypeSymbolKind::Enumeration);
    }

    #[test]
    fn classify_type_kind_when_function_block_then_returns_function_block() {
        let kind = classify_type_kind(&IntermediateType::FunctionBlock {
            name: "FB".to_string(),
            fields: vec![],
        });
        assert_eq!(kind, TypeSymbolKind::FunctionBlock);
    }

    #[test]
    fn type_symbol_kind_as_str_matches_mcp_contract() {
        assert_eq!(TypeSymbolKind::Enumeration.as_str(), "enumeration");
        assert_eq!(TypeSymbolKind::Structure.as_str(), "structure");
        assert_eq!(TypeSymbolKind::Array.as_str(), "array");
        assert_eq!(TypeSymbolKind::Subrange.as_str(), "subrange");
        assert_eq!(TypeSymbolKind::String.as_str(), "string");
        assert_eq!(TypeSymbolKind::Reference.as_str(), "reference");
        assert_eq!(TypeSymbolKind::Alias.as_str(), "alias");
    }

    #[test]
    fn variable_direction_as_str_matches_mcp_contract() {
        assert_eq!(VariableDirection::In.as_str(), "In");
        assert_eq!(VariableDirection::Out.as_str(), "Out");
        assert_eq!(VariableDirection::InOut.as_str(), "InOut");
        assert_eq!(VariableDirection::Local.as_str(), "Local");
        assert_eq!(VariableDirection::Global.as_str(), "Global");
        assert_eq!(VariableDirection::External.as_str(), "External");
    }

    #[test]
    fn extract_programs_when_program_declared_then_returned() {
        let ctx = analyze_source("PROGRAM p\nEND_PROGRAM");
        let programs = extract_programs(&ctx);
        assert_eq!(programs.len(), 1);
        assert_eq!(programs[0].name.to_string(), "p");
    }

    #[test]
    fn extract_programs_when_program_has_var_then_variable_returned() {
        let ctx = analyze_source("PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM");
        let programs = extract_programs(&ctx);
        assert_eq!(programs.len(), 1);
        let vars: Vec<_> = programs[0]
            .variables
            .iter()
            .map(|v| v.name.to_string().to_string())
            .collect();
        assert!(vars.contains(&"x".to_string()));
    }

    #[test]
    fn extract_function_blocks_when_fb_declared_then_returned() {
        let ctx = analyze_source(
            "FUNCTION_BLOCK fb\nVAR_INPUT i : INT; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR inst : fb; END_VAR\nEND_PROGRAM",
        );
        let fbs = extract_function_blocks(&ctx);
        let fb = fbs
            .iter()
            .find(|f| f.name.to_string() == "fb")
            .expect("fb missing");
        let input = fb
            .variables
            .iter()
            .find(|v| v.name.to_string() == "i")
            .expect("input missing");
        assert_eq!(input.direction, VariableDirection::In);
    }

    #[test]
    fn extract_user_defined_functions_when_function_declared_then_returned() {
        let ctx = analyze_source(
            "FUNCTION f : INT\nVAR_INPUT a : INT; END_VAR\nf := a;\nEND_FUNCTION\nPROGRAM p\nVAR r : INT; END_VAR\nr := f(a := 1);\nEND_PROGRAM",
        );
        let funcs = extract_user_defined_functions(&ctx);
        let f = funcs
            .iter()
            .find(|fv| fv.signature.name.to_string() == "f")
            .expect("function f missing");
        let return_type = f.return_type_name().expect("return type missing");
        assert_eq!(return_type.to_string().to_uppercase(), "INT");
        let params: Vec<_> = f.parameters().collect();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].direction, VariableDirection::In);
    }

    #[test]
    fn extract_user_defined_functions_when_only_stdlib_then_empty() {
        let ctx = analyze_source("PROGRAM p\nEND_PROGRAM");
        let funcs = extract_user_defined_functions(&ctx);
        assert!(funcs.is_empty(), "expected no user-defined functions");
    }

    #[test]
    fn extract_user_defined_types_when_enum_then_returned() {
        let ctx = analyze_source("TYPE\nMyEnum : (A, B, C);\nEND_TYPE\nPROGRAM p\nEND_PROGRAM");
        let types = extract_user_defined_types(&ctx);
        let t = types
            .iter()
            .find(|t| t.name.to_string().to_lowercase() == "myenum")
            .expect("MyEnum missing");
        assert_eq!(t.kind, TypeSymbolKind::Enumeration);
    }

    #[test]
    fn extract_user_defined_types_excludes_function_block_types() {
        let ctx = analyze_source(
            "FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR inst : fb; END_VAR\nEND_PROGRAM",
        );
        let types = extract_user_defined_types(&ctx);
        let has_fb_type = types.iter().any(|t| {
            matches!(
                t.attributes.representation,
                IntermediateType::FunctionBlock { .. }
            )
        });
        assert!(
            !has_fb_type,
            "function block types must not appear in user-defined types"
        );
    }
}
