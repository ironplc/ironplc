//! Transformation pass that resolves expression types.
//!
//! This pass populates the `resolved_type` field on `Expr` nodes. After this
//! pass, codegen can read types directly from expression nodes instead of
//! re-inferring them from variable names.
//!
//! The key problem this solves: codegen string-matches declared type names
//! (e.g., `"INT"`) against a hardcoded list. Type aliases like `"MyByte"`
//! don't match, causing incorrect opcode selection. By resolving aliases to
//! elementary types here, codegen gets clean type names.
use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use std::collections::HashMap;

use crate::function_environment::FunctionEnvironment;
use crate::type_environment::TypeEnvironment;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
    function_environment: &FunctionEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = ExprTypeResolver {
        var_types: HashMap::new(),
        type_environment,
        function_environment,
    };
    resolver.fold_library(lib).map_err(|e| vec![e])
}

struct ExprTypeResolver<'a> {
    /// Maps variable names to their declared TypeName within the current POU scope.
    var_types: HashMap<Id, TypeName>,
    type_environment: &'a TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
}

impl ExprTypeResolver<'_> {
    /// Extracts the TypeName from a variable declaration and inserts it into the
    /// variable type map.
    fn insert(&mut self, node: &VarDecl) {
        let type_name = match &node.initializer {
            InitialValueAssignmentKind::None(_) => return,
            InitialValueAssignmentKind::Simple(si) => si.type_name.clone(),
            InitialValueAssignmentKind::String(si) => si.type_name(),
            InitialValueAssignmentKind::EnumeratedValues(_) => return,
            InitialValueAssignmentKind::EnumeratedType(e) => e.type_name.clone(),
            InitialValueAssignmentKind::FunctionBlock(fb) => fb.type_name.clone(),
            InitialValueAssignmentKind::Subrange(spec) => match spec {
                SpecificationKind::Named(tn) => tn.clone(),
                SpecificationKind::Inline(sr) => TypeName::from(&sr.type_name.to_string()),
            },
            InitialValueAssignmentKind::Structure(s) => s.type_name.clone(),
            InitialValueAssignmentKind::Array(a) => match &a.spec {
                SpecificationKind::Named(tn) => tn.clone(),
                SpecificationKind::Inline(_) => return,
            },
            InitialValueAssignmentKind::LateResolvedType(tn) => tn.clone(),
        };

        match &node.identifier {
            VariableIdentifier::Symbol(id) => {
                self.var_types.insert(id.clone(), type_name);
            }
            VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    self.var_types.insert(name.clone(), type_name);
                }
            }
        }
    }

    /// Determines the resolved type for the given expression kind.
    fn resolve_type(&self, kind: &ExprKind) -> Option<TypeName> {
        match kind {
            ExprKind::Const(constant) => self.resolve_const_type(constant),
            ExprKind::Variable(var) => self.resolve_variable_type(var),
            ExprKind::BinaryOp(op) => op.left.resolved_type.clone(),
            ExprKind::UnaryOp(op) => op.term.resolved_type.clone(),
            ExprKind::Compare(_) => Some(TypeName::from("BOOL")),
            ExprKind::Function(f) => self
                .function_environment
                .get(&f.name)
                .and_then(|sig| sig.return_type.clone()),
            ExprKind::EnumeratedValue(ev) => ev.type_name.clone(),
            ExprKind::Expression(inner) => inner.resolved_type.clone(),
            ExprKind::LateBound(_) => None,
        }
    }

    fn resolve_const_type(&self, constant: &ConstantKind) -> Option<TypeName> {
        match constant {
            ConstantKind::IntegerLiteral(lit) => lit.data_type.as_ref().map(|itn| {
                let elem: ElementaryTypeName = itn.clone().into();
                elem.into()
            }),
            ConstantKind::RealLiteral(lit) => Some(
                lit.data_type
                    .as_ref()
                    .map(|rtn| {
                        let elem: ElementaryTypeName = rtn.clone().into();
                        let tn: TypeName = elem.into();
                        tn
                    })
                    .unwrap_or_else(|| TypeName::from("REAL")),
            ),
            ConstantKind::BitStringLiteral(lit) => lit.data_type.as_ref().map(|bstn| {
                let elem: ElementaryTypeName = bstn.clone().into();
                elem.into()
            }),
            ConstantKind::Boolean(_) => Some(TypeName::from("BOOL")),
            ConstantKind::CharacterString(_) => Some(TypeName::from("STRING")),
            ConstantKind::Duration(_) => Some(TypeName::from("TIME")),
            ConstantKind::TimeOfDay(_) => Some(TypeName::from("TIME_OF_DAY")),
            ConstantKind::Date(_) => Some(TypeName::from("DATE")),
            ConstantKind::DateAndTime(_) => Some(TypeName::from("DATE_AND_TIME")),
        }
    }

    fn resolve_variable_type(&self, var: &Variable) -> Option<TypeName> {
        match var {
            Variable::Symbolic(SymbolicVariableKind::Named(nv)) => {
                let declared = self.var_types.get(&nv.name)?;
                // Try to resolve to an elementary type. If the type is complex
                // (enum, struct, etc.), keep the declared name.
                Some(
                    self.type_environment
                        .resolve_elementary_type_name(declared)
                        .unwrap_or_else(|| declared.clone()),
                )
            }
            Variable::Symbolic(SymbolicVariableKind::Array(_)) => None,
            Variable::Symbolic(SymbolicVariableKind::Structured(_)) => None,
            Variable::Direct(_) => None,
        }
    }
}

impl Fold<Diagnostic> for ExprTypeResolver<'_> {
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.var_types.clear();
        result
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.var_types.clear();
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        let result = node.recurse_fold(self);
        self.var_types.clear();
        result
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        // First, recurse to fold children (bottom-up)
        let mut expr = node.recurse_fold(self)?;

        // Then determine type based on the (now-folded) kind
        expr.resolved_type = self.resolve_type(&expr.kind);
        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use crate::type_environment::TypeEnvironmentBuilder;
    use crate::xform_resolve_late_bound_expr_kind;
    use crate::xform_resolve_symbol_and_function_environment;
    use crate::xform_resolve_type_decl_environment;
    use crate::{
        function_environment::FunctionEnvironmentBuilder, symbol_environment::SymbolEnvironment,
    };
    use ironplc_dsl::common::Library;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::fold::Fold;
    use ironplc_dsl::textual::*;
    use ironplc_parser::options::ParseOptions;

    /// Runs the prerequisite passes and then the expression type resolution pass.
    fn run_pass(program: &str) -> Library {
        let library =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();
        let library =
            xform_resolve_type_decl_environment::apply(library, &mut type_environment).unwrap();
        let library =
            xform_resolve_late_bound_expr_kind::apply(library, &mut type_environment).unwrap();
        let mut function_environment = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        let mut symbol_environment = SymbolEnvironment::new();
        let library = xform_resolve_symbol_and_function_environment::apply(
            library,
            &mut symbol_environment,
            &mut function_environment,
        )
        .unwrap();
        apply(library, &mut type_environment, &function_environment).unwrap()
    }

    /// Helper visitor to collect resolved types from assignment RHS expressions.
    struct ResolvedTypeCollector {
        types: Vec<Option<ironplc_dsl::common::TypeName>>,
    }

    impl ResolvedTypeCollector {
        fn new() -> Self {
            Self { types: vec![] }
        }
    }

    impl Fold<()> for ResolvedTypeCollector {
        fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, ()> {
            self.types.push(node.value.resolved_type.clone());
            node.recurse_fold(self)
        }
    }

    /// Collects the resolved_type from the top-level assignment expressions.
    fn collect_assignment_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
        let mut collector = ResolvedTypeCollector::new();
        let _ = collector.fold_library(library.clone());
        collector.types
    }

    #[test]
    fn apply_when_simple_int_var_then_resolves_type() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "INT");
    }

    #[test]
    fn apply_when_type_alias_then_resolves_to_elementary() {
        let program = "
TYPE
    MyByte : BYTE := 0;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    x : MyByte;
    y : BYTE;
END_VAR
    y := x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "BYTE");
    }

    #[test]
    fn apply_when_bool_literal_then_resolves_bool() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    y : BOOL;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "BOOL");
    }

    #[test]
    fn apply_when_typed_integer_literal_then_resolves_type() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := INT#42;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "INT");
    }

    #[test]
    fn apply_when_comparison_then_resolves_bool() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x > 0;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "BOOL");
    }

    #[test]
    fn apply_when_binary_op_then_inherits_operand_type() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x + x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "INT");
    }

    #[test]
    fn apply_when_unary_op_then_inherits_operand_type() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := -x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_ref().unwrap().name.original(), "INT");
    }

    #[test]
    fn apply_when_function_call_then_resolves_return_type() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ABS(x);
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        // ABS returns the same type as input; the stdlib registers it with a return type
        assert!(types[0].is_some());
    }
}
