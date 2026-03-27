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

/// Returns true if the type name is an IEC 61131-3 generic type category.
///
/// These are abstract types used in stdlib function signatures that must be
/// resolved to concrete types based on the actual arguments.
fn is_generic_type(tn: &TypeName) -> bool {
    const GENERIC_TYPES: &[&str] = &[
        "ANY",
        "ANY_NUM",
        "ANY_REAL",
        "ANY_INT",
        "ANY_BIT",
        "ANY_STRING",
    ];
    GENERIC_TYPES.iter().any(|name| TypeName::from(name) == *tn)
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
            InitialValueAssignmentKind::Reference(ref_init) => match ref_init.target.type_name() {
                Some(tn) => tn.clone(),
                None => return, // Inline array targets don't have a single type name
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
            ExprKind::BinaryOp(op) => {
                match (&op.left.resolved_type, &op.right.resolved_type) {
                    // If left is generic and right is concrete, use the concrete type.
                    (Some(l), Some(r)) if is_generic_type(l) && !is_generic_type(r) => {
                        Some(r.clone())
                    }
                    (Some(l), _) => Some(l.clone()),
                    (_, r) => r.clone(),
                }
            }
            ExprKind::UnaryOp(op) => op.term.resolved_type.clone(),
            ExprKind::Compare(_) => Some(TypeName::from("BOOL")),
            ExprKind::Function(f) => {
                let sig = self.function_environment.get(&f.name)?;
                let return_type = sig.return_type.as_ref()?.to_type_name();
                if is_generic_type(&return_type) {
                    // Generic return type: infer concrete type from first argument
                    f.param_assignment.iter().find_map(|p| match p {
                        ParamAssignmentKind::PositionalInput(pos) => pos.expr.resolved_type.clone(),
                        ParamAssignmentKind::NamedInput(named) => named.expr.resolved_type.clone(),
                        _ => None,
                    })
                } else {
                    Some(return_type)
                }
            }
            ExprKind::EnumeratedValue(ev) => ev.type_name.clone(),
            ExprKind::Expression(inner) => inner.resolved_type.clone(),
            ExprKind::LateBound(_) => None,
            ExprKind::Ref(var) => {
                // REF(var) produces a reference — resolve the variable's type
                self.resolve_variable_type(var)
            }
            ExprKind::Deref(inner) => {
                // Dereference: the result type is the referenced variable's type.
                // The inner expression should be a reference whose resolved_type
                // is the referenced type name.
                inner.resolved_type.clone()
            }
            ExprKind::Null(_) => {
                // NULL has placeholder type BOOL (see design doc NULL Type Resolution Strategy).
                // Actual type compatibility is checked contextually by semantic rules.
                Some(TypeName::from("BOOL"))
            }
        }
    }

    fn resolve_const_type(&self, constant: &ConstantKind) -> Option<TypeName> {
        match constant {
            ConstantKind::IntegerLiteral(lit) => Some(
                lit.data_type
                    .as_ref()
                    .map(|itn| {
                        let elem: ElementaryTypeName = itn.clone().into();
                        let tn: TypeName = elem.into();
                        tn
                    })
                    .unwrap_or_else(|| TypeName::from("ANY_INT")),
            ),
            ConstantKind::RealLiteral(lit) => Some(
                lit.data_type
                    .as_ref()
                    .map(|rtn| {
                        let elem: ElementaryTypeName = rtn.clone().into();
                        let tn: TypeName = elem.into();
                        tn
                    })
                    .unwrap_or_else(|| TypeName::from("ANY_REAL")),
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
            Variable::Symbolic(SymbolicVariableKind::BitAccess(_)) => Some(TypeName::from("BOOL")),
            Variable::Symbolic(SymbolicVariableKind::Deref(_)) => None,
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
        // Register the implicit return variable (function name = return type)
        // so that references like `FOO := SHR(FOO, 1)` inside FUNCTION FOO
        // can resolve the type of the FOO variable.
        let return_type_name = node.return_type.to_type_name();
        let resolved_return_type = self
            .type_environment
            .resolve_elementary_type_name(&return_type_name)
            .unwrap_or(return_type_name);
        self.var_types
            .insert(node.name.clone(), resolved_return_type);
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

    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        if let InitialValueAssignmentKind::Simple(simple) = &node {
            if let Some(resolved) = self
                .type_environment
                .resolve_elementary_type_name(&simple.type_name)
            {
                return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: resolved,
                    initial_value: simple.initial_value.clone(),
                }));
            }
        }
        node.recurse_fold(self)
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
        assert_type_eq(&types[0], "INT");
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
        assert_type_eq(&types[0], "BYTE");
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
        assert_type_eq(&types[0], "BOOL");
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
        assert_type_eq(&types[0], "INT");
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
        assert_type_eq(&types[0], "BOOL");
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
        assert_type_eq(&types[0], "INT");
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
        assert_type_eq(&types[0], "INT");
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
        // ABS has generic return type ANY_NUM; should resolve to concrete input type
        assert_type_eq(&types[0], "INT");
    }

    #[test]
    fn apply_when_nested_function_call_then_resolves_concrete_type() {
        let program = "
PROGRAM test
  VAR
    a : DINT;
    result : DINT;
  END_VAR
    result := SHR(ABS(a), 1);
END_PROGRAM";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        // SHR has generic return type ANY_BIT; ABS(a) resolves to DINT,
        // so the outer SHR should also resolve to DINT
        assert_type_eq(&types[0], "DINT");
    }

    /// Collects resolved_type from every Expr node in the tree.
    struct AllExprTypeCollector {
        types: Vec<Option<ironplc_dsl::common::TypeName>>,
    }

    impl AllExprTypeCollector {
        fn new() -> Self {
            Self { types: vec![] }
        }
    }

    impl Fold<()> for AllExprTypeCollector {
        fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
            self.types.push(node.resolved_type.clone());
            node.recurse_fold(self)
        }
    }

    fn collect_all_expr_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
        let mut collector = AllExprTypeCollector::new();
        let _ = collector.fold_library(library.clone());
        collector.types
    }

    /// Returns the resolved type name as an uppercase &str for comparison.
    /// TypeEnvironment stores elementary types in lowercase, but IEC 61131-3
    /// type names are case-insensitive, so we normalize to uppercase for assertions.
    fn type_name_upper(tn: &Option<ironplc_dsl::common::TypeName>) -> Option<String> {
        tn.as_ref().map(|t| t.name.original().to_uppercase())
    }

    fn assert_type_eq(tn: &Option<ironplc_dsl::common::TypeName>, expected: &str) {
        assert_eq!(
            type_name_upper(tn),
            Some(expected.to_string()),
            "Expected type {expected}"
        );
    }

    #[test]
    fn apply_when_nested_arithmetic_then_all_subexprs_resolve() {
        // (x + y) * z — the inner (x + y) and outer multiply should all resolve to INT
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    result : INT;
END_VAR
    result := (x + y) * z;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_all_expr_types(&result);
        // Every expression node in (x + y) * z should resolve to INT:
        // nodes: result:=(expr), (x+y)*z, (x+y), x, y, z — all INT
        for (i, t) in types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type, got None"
            );
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_nested_comparison_with_arithmetic_then_resolves_correctly() {
        // (x + y) > z — inner (x + y) is INT, the comparison is BOOL
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    flag : BOOL;
END_VAR
    flag := (x + y) > z;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "BOOL");

        // Also verify inner nodes
        let all_types = collect_all_expr_types(&result);
        // The top-level expr is BOOL (comparison), but inner operands should be INT
        let has_bool = all_types
            .iter()
            .any(|t| type_name_upper(t) == Some("BOOL".to_string()));
        let has_int = all_types
            .iter()
            .any(|t| type_name_upper(t) == Some("INT".to_string()));
        assert!(has_bool, "Should have BOOL from comparison");
        assert!(has_int, "Should have INT from arithmetic operands");
    }

    #[test]
    fn apply_when_negated_nested_expr_then_resolves_type() {
        // -(x + y) — unary negation of a parenthesized binary op
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    result : INT;
END_VAR
    result := -(x + y);
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "INT");

        let all_types = collect_all_expr_types(&result);
        // All nodes (unary, parenthesized, binary, x, y) should be INT
        for (i, t) in all_types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type"
            );
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_deeply_nested_parens_then_resolves_type() {
        // ((x)) — multiple levels of parenthesization
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ((x));
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "INT");

        let all_types = collect_all_expr_types(&result);
        for (i, t) in all_types.iter().enumerate() {
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_bool_assigned_to_int_var_then_rhs_resolves_bool() {
        // The RHS expression type reflects the expression itself, not the target.
        // Here TRUE is BOOL even though the target y is INT.
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        // The expression TRUE resolves to BOOL regardless of the target variable type
        assert_type_eq(&types[0], "BOOL");
    }

    #[test]
    fn apply_when_int_assigned_to_bool_var_then_rhs_resolves_int() {
        // The expression type is determined by the expression, not the target.
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "INT");
    }

    #[test]
    fn apply_when_mixed_type_binary_op_then_inherits_left_operand_type() {
        // In x + y where x is DINT and y is INT, the result inherits the left operand type.
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : DINT;
    y : INT;
    result : DINT;
END_VAR
    result := x + y;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "DINT");
    }

    #[test]
    fn apply_when_binary_op_literal_plus_concrete_then_resolves_concrete() {
        // In 5 + x where x is INT, the result should be INT (concrete wins over ANY_INT)
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := 5 + x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "INT");
    }

    #[test]
    fn apply_when_binary_op_concrete_plus_literal_then_resolves_concrete() {
        // In x + 5 where x is INT, the result should be INT (left is concrete)
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := x + 5;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "INT");
    }

    #[test]
    fn apply_when_binary_op_two_literals_then_resolves_any_int() {
        // In 5 + 10, both are ANY_INT, so the result is ANY_INT
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    result : DINT;
END_VAR
    result := 5 + 10;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "ANY_INT");
    }

    #[test]
    fn apply_when_real_literal_without_type_then_resolves_any_real() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    y : REAL;
END_VAR
    y := 3.14;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "ANY_REAL");
    }

    #[test]
    fn apply_when_subrange_var_then_resolves_base_type() {
        // A subrange variable like INT(-100..100) should resolve to INT.
        let program = "
FUNCTION_BLOCK FB_TEST
VAR_IN_OUT
    x : INT(-100..100);
END_VAR
VAR
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "INT");
    }

    #[test]
    fn apply_when_multiple_assignments_then_each_resolves_independently() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
    a : INT;
    b : BOOL;
END_VAR
    a := x;
    b := y;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 2);
        assert_type_eq(&types[0], "INT");
        assert_type_eq(&types[1], "BOOL");
    }

    #[test]
    fn apply_when_string_literal_then_resolves_string() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    s : STRING;
END_VAR
    s := 'hello';
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "STRING");
    }

    #[test]
    fn apply_when_untyped_integer_literal_then_resolves_any_int() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    y : DINT;
END_VAR
    y := 42;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "ANY_INT");
    }

    #[test]
    fn apply_when_function_return_var_used_in_builtin_then_resolves_type() {
        let program = "
FUNCTION FOO : INT
  VAR_INPUT
    A : INT;
  END_VAR
  FOO := 8;
  FOO := SHR(FOO, 1);
END_FUNCTION

PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := FOO(A := 5);
END_PROGRAM";

        let result = run_pass(program);
        let all_types = collect_all_expr_types(&result);
        // Every expression node should have a resolved type (no None values)
        for (i, t) in all_types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type, got None"
            );
        }
    }

    #[test]
    fn apply_when_time_literal_then_resolves_time() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    t : TIME;
END_VAR
    t := T#5s;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], "TIME");
    }
}
