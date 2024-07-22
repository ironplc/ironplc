//! Transformation rule that changes late bound expression elements
//! specific types.
//!
//! Late bound types are those where the type is ambiguous until
//! after parsing.
//!
//! The transformation succeeds when all ambiguous expression elements
//! resolve to a declared type.
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_dsl::{common::*, core::Id};
use std::collections::HashMap;

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    // Resolve the types. This is a single fold of the library
    let mut resolver = DeclarationResolver {
        names_to_types: HashMap::new(),
        current_type: VariableType::None,
        diagnostics: Vec::new(),
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

struct DeclarationResolver {
    // Defines the desired type for each identifier
    names_to_types: HashMap<Id, VariableType>,
    current_type: VariableType,
    diagnostics: Vec<Diagnostic>,
}

impl DeclarationResolver {
    fn insert(&mut self, node: &VarDecl) {
        let var_type = match node.initializer {
            InitialValueAssignmentKind::None(_) => VariableType::None,
            InitialValueAssignmentKind::Simple(_) => VariableType::Simple,
            InitialValueAssignmentKind::String(_) => VariableType::String,
            InitialValueAssignmentKind::EnumeratedValues(_) => VariableType::EnumeratedValues,
            InitialValueAssignmentKind::EnumeratedType(_) => VariableType::EnumeratedType,
            InitialValueAssignmentKind::FunctionBlock(_) => VariableType::FunctionBlock,
            InitialValueAssignmentKind::Subrange(_) => VariableType::Subrange,
            InitialValueAssignmentKind::Structure(_) => VariableType::Structure,
            InitialValueAssignmentKind::Array(_) => VariableType::Array,
            InitialValueAssignmentKind::LateResolvedType(_) => VariableType::LateResolvedType,
        };
        match &node.identifier {
            VariableIdentifier::Symbol(id) => {
                self.names_to_types.insert(id.clone(), var_type);
            }
            VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    self.names_to_types.insert(name.clone(), var_type);
                }
            }
        }
    }

    fn find_type(&self, name: &Id) -> &VariableType {
        self.names_to_types.get(name).unwrap_or(&VariableType::None)
    }
}

impl Fold<Diagnostic> for DeclarationResolver {
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.names_to_types.clear();
        result
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.names_to_types.clear();
        result
    }
    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.names_to_types.clear();
        result
    }
    fn fold_assignment(
        &mut self,
        node: ironplc_dsl::textual::Assignment,
    ) -> Result<ironplc_dsl::textual::Assignment, Diagnostic> {
        // Check the type of the target. We will later use that to assign
        // any late types in the expression.
        match &node.target {
            Variable::Direct(_) => self.current_type = VariableType::None,
            Variable::Symbolic(symbolic_kind) => {
                match symbolic_kind {
                    SymbolicVariableKind::Named(named) => {
                        // An assignment to a named variable. Look up the variable
                        // that we should have found earlier to identify the type
                        self.current_type = self.find_type(&named.name).clone();
                    }
                    SymbolicVariableKind::Array(arr) => {
                        Err(Diagnostic::todo_with_span(arr.span(), file!(), line!()))?
                    }
                    SymbolicVariableKind::Structured(st) => {
                        Err(Diagnostic::todo_with_span(st.span(), file!(), line!()))?
                    }
                }
            }
        }

        // Now recurse into the node so that we can replace any late bound expression elements
        let result = node.recurse_fold(self);

        // Done with this assignment, so reset the current type
        self.current_type = VariableType::None;

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
            ExprKind::LateBound(node) => match self.current_type {
                VariableType::None => {
                    // TODO this is likely not right in all cases
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.name }),
                    )))
                }
                VariableType::Simple => Ok(ExprKind::Variable(Variable::Symbolic(
                    SymbolicVariableKind::Named(NamedVariable { name: node.name }),
                ))),
                VariableType::String => Err(Diagnostic::todo(file!(), line!())),
                VariableType::EnumeratedValues => Err(Diagnostic::todo(file!(), line!())),
                VariableType::EnumeratedType => Ok(ExprKind::EnumeratedValue(EnumeratedValue {
                    type_name: None,
                    value: node.name,
                })),
                VariableType::FunctionBlock => Err(Diagnostic::todo(file!(), line!())),
                VariableType::Subrange => Err(Diagnostic::todo(file!(), line!())),
                VariableType::Structure => Err(Diagnostic::todo(file!(), line!())),
                VariableType::Array => Ok(ExprKind::Variable(Variable::Symbolic(
                    SymbolicVariableKind::Named(NamedVariable { name: node.name }),
                ))),
                VariableType::LateResolvedType => Err(Diagnostic::todo(file!(), line!())),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use ironplc_dsl::core::FileId;

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

        let library = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        print!("{:?}", library);
        let result = apply(library);

        assert!(result.is_ok());
    }
}
