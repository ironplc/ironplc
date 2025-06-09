//! Transformation rule that changes late bound expression elements
//! specific types.
//!
//! Late bound types are those where the type is ambiguous until
//! after parsing.
//!
//! The transformation succeeds when all ambiguous expression elements
//! resolve to a declared type.
use std::collections::HashMap;

use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_dsl::{common::*, core::Id};

use crate::type_environment::{TypeClass, TypeEnvironment};

pub fn apply(
    lib: Library,
    type_environment: &TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    // Resolve the types. This is a single fold of the library
    let mut resolver = DeclarationResolver {
        diagnostics: Vec::new(),
        type_environment,
        assignment_target: None,
        variables: HashMap::new(),
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e]);

    if !resolver.diagnostics.is_empty() {
        return Err(resolver.diagnostics);
    }

    result
}

#[derive(Clone)]
enum VariableType {
    None,
    Simple,
    String,
    EnumeratedValues,
    EnumeratedType,
    FunctionBlock,
    Subrange,
    Structure,
    Array,
    LateResolvedType,
}

struct DeclarationResolver<'a> {
    assignment_target: Option<Variable>,
    variables: HashMap<Id, Type>,
    diagnostics: Vec<Diagnostic>,
    type_environment:  &'a TypeEnvironment,
}

impl<'a> DeclarationResolver<'a> {
    fn insert(&mut self, node: &VarDecl) {
        // Gets the name of the type for this variable.
        if let Some(type_name) = node.type_name() {
            if let VariableIdentifier::Symbol(name) = &node.identifier {
                // Add this mapping into context
                self.variables.insert(name.clone(), type_name);
            }
        }
    }

    fn get_type(&self, target: &SymbolicVariableKind) -> Result<&Type, Diagnostic> {
        // Get the variable name
        let variable: Option<&Type> = match target {
            SymbolicVariableKind::Named(named_var) => {
                // If this is just a variable name, then we have the type already
                // from the scope (unless this is global?)
                return self.variables.get(&named_var.name).ok_or_else(|| {
                    let diagnostic = Diagnostic::todo_with_id(&named_var.name, file!(), line!());
                    for var in self.variables {
                        diagnostic = diagnostic.with_secondary_id(var.0.span(), "Variable");
                    }
                    diagnostic
                        
                });
            },
            SymbolicVariableKind::Array(arr) => {
                // If this is array, then we need to look up based on the container
                return Err(Diagnostic::todo_with_span(arr.span(), file!(), line!()));
            },
            SymbolicVariableKind::Structured(structure) => {
                // For now, just go one level into a structure
                if let SymbolicVariableKind::Named(container) = structure.record.as_ref() {
                    // This is the name of the variable
                    return self.variables.get(&container.name);
                }
                return Err(Diagnostic::todo_with_span(structure.span(), file!(), line!()));
            }
        };
    }

    fn replace_late_bound_expr(&self, node: LateBound) -> Result<ironplc_dsl::textual::ExprKind, Diagnostic> {
        // What variable are we trying to assign this value to?
        let symbolic_type = match self.assignment_target.as_ref() {
            Some(target_variable) => {
                // We need this to be a symbolic variable to resolve the type.
                let target_symbolic_variable = match target_variable {
                    Variable::Direct(address_assignment) => return Err(Diagnostic::todo_with_id(&node.value, file!(), line!())),
                    Variable::Symbolic(symbolic_variable_kind) => symbolic_variable_kind,
                };
                let target_type = self.get_type(&target_symbolic_variable)?;
                target_type
            },
            None => {
                // This might be a direct variable reference.
                let variable_type = self.variables.get(&node.value).ok_or_else(|| Diagnostic::todo_with_id(&node.value, file!(), line!()))?;
                variable_type
            },
        };

        let attrs = self.type_environment.get(symbolic_type).ok_or_else(|| Diagnostic::todo_with_id(&node.value, file!(), line!()))?;

        let replaced_type = match attrs.class {
            TypeClass::Simple => Ok(ExprKind::Variable(Variable::Symbolic(
                SymbolicVariableKind::Named(NamedVariable { name: node.value }),
            ))),
            TypeClass::Enumeration => Ok(ExprKind::EnumeratedValue(EnumeratedValue {
                type_name: None,
                value: node.value,
            })),
            TypeClass::Structure => Err(Diagnostic::todo_with_id(&node.value, file!(), line!())),
            TypeClass::FunctionBlock => Err(Diagnostic::todo_with_id(&node.value, file!(), line!())),
            TypeClass::FunctionBlockOutput(_) => Err(Diagnostic::todo_with_id(&node.value, file!(), line!())),
        };

        replaced_type
    }
}

impl<'a> Fold<Diagnostic> for DeclarationResolver<'a> {
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        self.variables = HashMap::new();
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.variables.clear();
        result
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        self.variables = HashMap::new();
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.variables.clear();
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        self.variables = HashMap::new();
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.variables.clear();
        result
    }

    fn fold_assignment(
        &mut self,
        node: ironplc_dsl::textual::Assignment,
    ) -> Result<ironplc_dsl::textual::Assignment, Diagnostic> {
        // Keep track of what we are assigning to. That can help figure out the types in the expression.
        self.assignment_target = Some(node.target.clone());

        // Now recurse into the node so that we can replace any late bound expression elements
        let result = node.recurse_fold(self);

        // Done with this assignment, so reset the current type
        self.assignment_target = None;

        result
    }

    fn fold_expr_kind(
        &mut self,
        node: ironplc_dsl::textual::ExprKind,
    ) -> Result<ironplc_dsl::textual::ExprKind, Diagnostic> {
        match node {
            ExprKind::Compare(node) => node
                .recurse_fold(self)
                .map(|v| Ok(ExprKind::Compare(Box::new(v))))?,
            ExprKind::BinaryOp(node) => node
                .recurse_fold(self)
                .map(|v| Ok(ExprKind::BinaryOp(Box::new(v))))?,
            ExprKind::UnaryOp(node) => node
                .recurse_fold(self)
                .map(|v| Ok(ExprKind::UnaryOp(Box::new(v))))?,
            ExprKind::Expression(node) => node
                .recurse_fold(self)
                .map(|v| Ok(ExprKind::Expression(Box::new(v))))?,
            ExprKind::Const(node) => node
                .recurse_fold(self)
                .map(|v: ConstantKind| Ok(ExprKind::Const(v)))?,
            ExprKind::EnumeratedValue(node) => node
                .recurse_fold(self)
                .map(|v| Ok(ExprKind::EnumeratedValue(v)))?,
            ExprKind::Variable(node) => {
                node.recurse_fold(self).map(|v| Ok(ExprKind::Variable(v)))?
            }
            ExprKind::Function(node) => {
                node.recurse_fold(self).map(|v| Ok(ExprKind::Function(v)))?
            }
            ExprKind::LateBound(node) => {
                self.replace_late_bound_expr(node)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::type_environment::TypeEnvironmentBuilder;

    use super::apply;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::ParseOptions;

    #[test]
    fn apply_when_assign_enum_variant_then_ok() {
        let program = "
TYPE
    MyColors: (Red, Green);
END_TYPE

FUNCTION_BLOCK FB_EXAMPLE
    VAR
        Color: MyColors := Red;
    END_VAR
    Color := Green;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        println!("{:?}", &library);
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        println!("{:?}", &result.unwrap());

        //assert!(result.is_ok());
    }

    #[test]
    fn apply_when_assign_to_array_member() {
        // TODO this fails
        let program = "FUNCTION_BLOCK _BUFFER_INSERT

VAR_IN_OUT
	data : ARRAY[1..2] OF INT; 
END_VAR

VAR
	i :	INT;
	i2 : INT;
END_VAR
			
data[i] := data[i2]; 

END_FUNCTION_BLOCK
";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _ = apply(library, &mut type_environment);

        //assert!(result.is_ok());
    }
}
