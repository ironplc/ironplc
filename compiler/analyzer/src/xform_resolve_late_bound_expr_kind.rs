//! Transformation rule that changes late bound expression elements
//! specific types.
//!
//! Late bound types are those where the type is ambiguous until
//! after parsing.
//!
//! The transformation succeeds when all ambiguous expression elements
//! resolve to a declared type.
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_dsl::{common::*, core::Id};
use std::collections::HashMap;

use crate::type_environment::TypeEnvironment;

pub fn apply(
    lib: Library,
    _type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
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
                    SymbolicVariableKind::Array(_arr) => {
                        // Assignment to an array element like data[i] := value
                        // Determining the element type requires looking up the array
                        // declaration which isn't available here. Default to None.
                        self.current_type = VariableType::None;
                    }
                    SymbolicVariableKind::Structured(_st) => {
                        // Assignment to a structure member like s.field := value
                        // Determining the field type requires looking up the struct
                        // declaration which isn't available here. Default to None.
                        self.current_type = VariableType::None;
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
                    // When no type information is available, assume a variable reference
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                    )))
                }
                VariableType::Simple => Ok(ExprKind::Variable(Variable::Symbolic(
                    SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                ))),
                VariableType::String => {
                    // String assignment from another string variable
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                    )))
                }
                VariableType::EnumeratedValues => {
                    // Inline enumeration like (Red, Green) - the value is an enum constant
                    Ok(ExprKind::EnumeratedValue(EnumeratedValue {
                        type_name: None,
                        value: node.value,
                    }))
                }
                VariableType::EnumeratedType => Ok(ExprKind::EnumeratedValue(EnumeratedValue {
                    type_name: None,
                    value: node.value,
                })),
                VariableType::FunctionBlock => {
                    // Function block variables are parsed as LateResolvedType, not FunctionBlock.
                    // If we reach this branch, it indicates an internal error.
                    Err(Diagnostic::internal_error(file!(), line!()))
                }
                VariableType::Subrange => {
                    // Subrange assignment from another integer variable
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                    )))
                }
                VariableType::Structure => {
                    // Structure assignment from another structure variable
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                    )))
                }
                VariableType::Array => Ok(ExprKind::Variable(Variable::Symbolic(
                    SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                ))),
                VariableType::LateResolvedType => {
                    // The variable type hasn't been resolved yet. Since we don't know if
                    // this is an enum or another type, default to variable reference.
                    // Semantic analysis will catch type mismatches later.
                    Ok(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: node.value }),
                    )))
                }
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
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_assign_to_array_member_then_ok() {
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
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: String variable type - assignment from another variable
    // Does this trigger VariableType::String?
    #[test]
    fn apply_when_string_var_assign_from_other_var() {
        let program = "
FUNCTION_BLOCK FB_TEST
    VAR
        s1 : STRING;
        s2 : STRING;
    END_VAR
    s1 := s2;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: Inline enumerated values
    // Does this trigger VariableType::EnumeratedValues?
    #[test]
    fn apply_when_inline_enum_assign_value() {
        let program = "
FUNCTION_BLOCK FB_TEST
    VAR
        Color : (Red, Green, Blue);
    END_VAR
    Color := Green;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: Subrange type - inline subrange in VAR_IN_OUT
    // Does this trigger VariableType::Subrange?
    #[test]
    fn apply_when_subrange_assign_from_var() {
        // VAR_IN_OUT allows inline subrange specification
        let program = "
FUNCTION_BLOCK FB_TEST
VAR_IN_OUT
    x : INT(-100..100);
END_VAR
VAR
    y : INT;
END_VAR
    x := y;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: Structure type
    // Does this trigger VariableType::Structure?
    #[test]
    fn apply_when_structure_assign_from_var() {
        let program = "
TYPE
    MyStruct : STRUCT
        field : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB_TEST
    VAR
        s1 : MyStruct;
        s2 : MyStruct;
    END_VAR
    s1 := s2;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: LateResolvedType - enum without initializer
    // Does this trigger VariableType::LateResolvedType?
    #[test]
    fn apply_when_enum_without_init_assign_value() {
        let program = "
TYPE
    MyColors: (Red, Green);
END_TYPE

FUNCTION_BLOCK FB_TEST
    VAR
        Color: MyColors;
    END_VAR
    Color := Green;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        assert!(result.is_ok());
    }

    // Test: Function block assignment.
    // Note: Function block variables are parsed as LateResolvedType (not FunctionBlock),
    // so this exercises the LateResolvedType handling which treats the RHS as a variable.
    #[test]
    fn apply_when_function_block_assign_from_var_then_ok() {
        let program = "
FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_TEST
    VAR
        fb1 : MyFB;
        fb2 : MyFB;
    END_VAR
    fb1 := fb2;
END_FUNCTION_BLOCK";

        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(library, &mut type_environment);
        // This succeeds because FB variables are parsed as LateResolvedType,
        // and we handle that by treating the RHS as a variable reference.
        // Semantic analysis later will catch invalid FB assignments.
        assert!(result.is_ok());
    }
}
