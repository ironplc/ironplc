//! Semantic rules for REF_TO reference types.
//!
//! This module validates the usage of REF_TO, REF(), NULL, and the dereference
//! operator (^) according to IEC 61131-3 Edition 3 safety constraints.

use ironplc_dsl::{
    common::*,
    core::{Id, Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashMap;

use crate::{
    result::SemanticResult, semantic_context::SemanticContext, type_environment::TypeEnvironment,
};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleRefTo {
        type_environment: context.types(),
        var_types: HashMap::new(),
        var_classes: HashMap::new(),
        pou_kind: PouKind::Program,
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum PouKind {
    Function,
    FunctionBlock,
    Program,
}

struct RuleRefTo<'a> {
    type_environment: &'a TypeEnvironment,
    /// Maps variable names to their initializer kind within the current POU scope.
    var_types: HashMap<Id, InitialValueAssignmentKind>,
    /// Maps variable names to their variable class (VAR, VAR_TEMP, VAR_INPUT, etc.)
    var_classes: HashMap<Id, VariableType>,
    /// The kind of POU currently being visited.
    pou_kind: PouKind,
    diagnostics: Vec<Diagnostic>,
}

/// Extracts a span from a Variable, falling back to default.
fn variable_span(var: &Variable) -> SourceSpan {
    match var {
        Variable::Symbolic(sym) => sym.span(),
        Variable::Direct(_) => SourceSpan::default(),
    }
}

impl RuleRefTo<'_> {
    fn collect_variables(&mut self, variables: &[VarDecl]) {
        for var in variables {
            if let VariableIdentifier::Symbol(id) = &var.identifier {
                self.var_types.insert(id.clone(), var.initializer.clone());
                self.var_classes.insert(id.clone(), var.var_type.clone());
            }
        }
    }

    fn clear_variables(&mut self) {
        self.var_types.clear();
        self.var_classes.clear();
    }

    /// Returns the TypeName for a variable's declared type, if it can be resolved.
    fn variable_type_name(&self, var: &Variable) -> Option<TypeName> {
        let id = match var {
            Variable::Symbolic(SymbolicVariableKind::Named(named)) => &named.name,
            Variable::Symbolic(SymbolicVariableKind::Structured(s)) => match s.record.as_ref() {
                SymbolicVariableKind::Named(named) => &named.name,
                _ => return None,
            },
            _ => return None,
        };
        let init = self.var_types.get(id)?;
        match init {
            InitialValueAssignmentKind::Simple(si) => Some(si.type_name.clone()),
            InitialValueAssignmentKind::Reference(ri) => ri.target.type_name().cloned(),
            InitialValueAssignmentKind::LateResolvedType(tn) => Some(tn.clone()),
            _ => None,
        }
    }

    /// Returns true if the given type name resolves to a reference type.
    fn is_reference_type(&self, type_name: &TypeName) -> bool {
        self.type_environment
            .get(type_name)
            .map(|attrs| attrs.representation.is_reference())
            .unwrap_or(false)
    }

    /// Returns true if the variable is declared as REF_TO.
    fn is_variable_reference(&self, var: &Variable) -> bool {
        let id = match var {
            Variable::Symbolic(SymbolicVariableKind::Named(named)) => &named.name,
            _ => return false,
        };
        match self.var_types.get(id) {
            Some(InitialValueAssignmentKind::Reference(_)) => true,
            Some(InitialValueAssignmentKind::Simple(si)) => self.is_reference_type(&si.type_name),
            Some(InitialValueAssignmentKind::LateResolvedType(tn)) => self.is_reference_type(tn),
            _ => false,
        }
    }

    /// Returns true if the expression resolves to a reference type.
    fn is_expr_reference(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ref(_) => true,
            ExprKind::Null(_) => true,
            ExprKind::Variable(var) => self.is_variable_reference(var),
            _ => {
                if let Some(ref resolved) = expr.resolved_type {
                    self.is_reference_type(resolved)
                } else {
                    false
                }
            }
        }
    }

    /// P2028: REF() operand must be a simple named variable
    fn check_ref_operand(&mut self, var: &Variable) {
        let span = variable_span(var);
        match var {
            Variable::Symbolic(SymbolicVariableKind::Named(_)) => {
                // Simple named variable — OK, check for ephemeral below
            }
            Variable::Symbolic(SymbolicVariableKind::Array(_)) => {
                // P2030: REF of array element
                self.diagnostics.push(Diagnostic::problem(
                    Problem::RefOfArrayElement,
                    Label::span(span, "REF() of array element is not supported"),
                ));
                return;
            }
            _ => {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::RefOperandNotVariable,
                    Label::span(span, "REF() operand must be a simple variable"),
                ));
                return;
            }
        }

        // P2029: Check for ephemeral variables
        if let Variable::Symbolic(SymbolicVariableKind::Named(named)) = var {
            if let Some(var_class) = self.var_classes.get(&named.name) {
                match var_class {
                    VariableType::VarTemp => {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::RefOfEphemeralVariable,
                            Label::span(named.span(), "VAR_TEMP variable is stack-allocated"),
                        ));
                    }
                    VariableType::Input | VariableType::Output
                        if self.pou_kind == PouKind::Function =>
                    {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::RefOfEphemeralVariable,
                            Label::span(named.span(), "FUNCTION parameter is stack-allocated"),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    /// P2031: Dereference requires reference type
    fn check_deref(&mut self, inner: &Expr) {
        if let ExprKind::Variable(var) = &inner.kind {
            if !self.is_variable_reference(var) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::DerefRequiresReferenceType,
                    Label::span(
                        variable_span(var),
                        "Dereference operator (^) requires a REF_TO type",
                    ),
                ));
            }
        }
    }

    /// P2033: No arithmetic on reference types
    fn check_binary_op(&mut self, expr: &BinaryExpr) {
        let left_ref = self.is_expr_reference(&expr.left);
        let right_ref = self.is_expr_reference(&expr.right);
        if left_ref || right_ref {
            let span = if left_ref {
                expr_span(&expr.left)
            } else {
                expr_span(&expr.right)
            };
            self.diagnostics.push(Diagnostic::problem(
                Problem::ArithmeticOnReference,
                Label::span(
                    span,
                    "Arithmetic operations are not allowed on reference types",
                ),
            ));
        }
    }

    /// P2035: Only = and <> on references
    fn check_compare_op(&mut self, expr: &CompareExpr) {
        let left_ref = self.is_expr_reference(&expr.left);
        let right_ref = self.is_expr_reference(&expr.right);
        if !left_ref && !right_ref {
            return;
        }
        match expr.op {
            CompareOp::Eq | CompareOp::Ne => {
                // Equality and inequality are allowed on references
            }
            CompareOp::Lt | CompareOp::Gt | CompareOp::LtEq | CompareOp::GtEq => {
                let span = if left_ref {
                    expr_span(&expr.left)
                } else {
                    expr_span(&expr.right)
                };
                self.diagnostics.push(Diagnostic::problem(
                    Problem::OrderingOnReference,
                    Label::span(span, "Ordering comparison on reference types"),
                ));
            }
            CompareOp::Or | CompareOp::Xor | CompareOp::And => {}
        }
    }

    /// P2034: NULL can only be assigned to REF_TO type
    fn check_null_assignment(&mut self, target: &Variable, value: &Expr) {
        if let ExprKind::Null(span) = &value.kind {
            if !self.is_variable_reference(target) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::NullRequiresReferenceType,
                    Label::span(span.clone(), "NULL can only be assigned to a REF_TO type"),
                ));
            }
        }
    }

    /// P2032: Reference type mismatch in assignment
    fn check_ref_assignment(&mut self, target: &Variable, value: &Expr) {
        if let ExprKind::Ref(ref_var) = &value.kind {
            let ref_span = variable_span(ref_var);
            if !self.is_variable_reference(target) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::ReferenceTypeMismatch,
                    Label::span(ref_span, "Reference type mismatch in assignment"),
                ));
            } else {
                // Check type compatibility
                let target_ref_type = self.get_reference_target_type(target);
                let operand_type = self.variable_type_name(ref_var);
                if let (Some(target_type), Some(operand_type)) = (target_ref_type, operand_type) {
                    if target_type != operand_type {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::ReferenceTypeMismatch,
                            Label::span(ref_span, "Reference type mismatch in assignment"),
                        ));
                    }
                }
            }
        }
    }

    /// Returns the target type of a REF_TO variable.
    fn get_reference_target_type(&self, var: &Variable) -> Option<TypeName> {
        let id = match var {
            Variable::Symbolic(SymbolicVariableKind::Named(named)) => &named.name,
            _ => return None,
        };
        match self.var_types.get(id)? {
            InitialValueAssignmentKind::Reference(ri) => ri.target.type_name().cloned(),
            _ => None,
        }
    }
}

/// Extracts a best-effort span from an Expr.
fn expr_span(expr: &Expr) -> SourceSpan {
    match &expr.kind {
        ExprKind::Variable(var) => variable_span(var),
        ExprKind::Ref(var) => variable_span(var),
        ExprKind::Null(span) => span.clone(),
        _ => SourceSpan::default(),
    }
}

impl Visitor<Diagnostic> for RuleRefTo<'_> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.pou_kind = PouKind::Function;
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.pou_kind = PouKind::FunctionBlock;
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.pou_kind = PouKind::Program;
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_reference_declaration(
        &mut self,
        node: &ReferenceDeclaration,
    ) -> Result<(), Diagnostic> {
        // P2036: Check for nested REF_TO (only applicable for named targets)
        if let ReferenceTarget::Named(referenced_type_name) = &node.target {
            if self.is_reference_type(referenced_type_name) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::NestedRefToNotSupported,
                    Label::span(node.type_name.span(), "Nested REF_TO is not supported"),
                ));
            }
        }
        node.recurse_visit(self)
    }

    fn visit_expr(&mut self, node: &Expr) -> Result<(), Diagnostic> {
        match &node.kind {
            ExprKind::Ref(var) => {
                self.check_ref_operand(var);
            }
            ExprKind::Deref(inner) => {
                self.check_deref(inner);
            }
            ExprKind::BinaryOp(binary) => {
                self.check_binary_op(binary);
            }
            ExprKind::Compare(compare) => {
                self.check_compare_op(compare);
            }
            _ => {}
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Diagnostic> {
        self.check_null_assignment(&node.target, &node.value);
        self.check_ref_assignment(&node.target, &node.value);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::ParseOptions, parse_program};

    fn parse_edition3(program: &str) -> Result<(), String> {
        let options = ParseOptions {
            allow_iec_61131_3_2013: true,
            ..ParseOptions::default()
        };
        let library =
            parse_program(program, &FileId::default(), &options).map_err(|e| format!("{e:?}"))?;
        let (_library, context) = analyze(&[&library]).map_err(|e| format!("{e:?}"))?;
        if context.has_diagnostics() {
            Err(format!("{:?}", context.diagnostics()))
        } else {
            Ok(())
        }
    }

    fn assert_ok(program: &str) {
        let result = parse_edition3(program);
        assert!(result.is_ok(), "Expected OK but got: {:?}", result.err());
    }

    fn assert_err(program: &str) {
        let result = parse_edition3(program);
        assert!(result.is_err(), "Expected error but got OK");
    }

    // P2036: No nested REF_TO
    #[test]
    fn ref_to_when_single_level_then_ok() {
        assert_ok(
            "TYPE IntRef : REF_TO INT; END_TYPE
PROGRAM Main
VAR
    x : INT;
    r : IntRef;
END_VAR
    r := REF(x);
END_PROGRAM",
        );
    }

    // P2028: REF() operand must be a simple variable
    #[test]
    fn ref_when_operand_is_named_variable_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    x : INT;
    r : REF_TO INT;
END_VAR
    r := REF(x);
END_PROGRAM",
        );
    }

    // P2029: No REF of ephemeral variables - VAR_TEMP
    #[test]
    fn ref_when_operand_is_var_temp_then_error() {
        assert_err(
            "FUNCTION_BLOCK FB1
VAR_TEMP
    temp : INT;
END_VAR
VAR
    r : REF_TO INT;
END_VAR
    r := REF(temp);
END_FUNCTION_BLOCK",
        );
    }

    // P2029: No REF of FUNCTION VAR_INPUT
    #[test]
    fn ref_when_operand_is_function_var_input_then_error() {
        assert_err(
            "FUNCTION MyFunc : INT
VAR_INPUT
    inVal : INT;
END_VAR
VAR
    r : REF_TO INT;
END_VAR
    r := REF(inVal);
    MyFunc := 0;
END_FUNCTION",
        );
    }

    // P2029: FB VAR_INPUT is persistent — OK
    #[test]
    fn ref_when_operand_is_fb_var_input_then_ok() {
        assert_ok(
            "FUNCTION_BLOCK FB1
VAR_INPUT
    inVal : INT;
END_VAR
VAR
    r : REF_TO INT;
END_VAR
    r := REF(inVal);
END_FUNCTION_BLOCK",
        );
    }

    // P2030: No REF of array elements
    #[test]
    fn ref_when_operand_is_array_element_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    arr : ARRAY [0..9] OF INT;
    r : REF_TO INT;
END_VAR
    r := REF(arr[3]);
END_PROGRAM",
        );
    }

    // P2031: Deref requires reference type
    #[test]
    fn deref_when_type_is_not_reference_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    x : INT := 42;
    y : INT;
END_VAR
    y := x^;
END_PROGRAM",
        );
    }

    #[test]
    fn deref_when_type_is_reference_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    x : INT;
    r : REF_TO INT := REF(x);
    y : INT;
END_VAR
    y := r^;
END_PROGRAM",
        );
    }

    // P2033: No arithmetic on references
    #[test]
    fn arithmetic_when_operand_is_reference_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    x : INT;
    r : REF_TO INT := REF(x);
    y : INT;
END_VAR
    y := r + 1;
END_PROGRAM",
        );
    }

    // P2034: NULL only for reference types
    #[test]
    fn null_when_assigned_to_non_reference_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    x : INT;
END_VAR
    x := NULL;
END_PROGRAM",
        );
    }

    #[test]
    fn null_when_assigned_to_reference_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    x : INT;
    r : REF_TO INT := REF(x);
END_VAR
    r := NULL;
END_PROGRAM",
        );
    }

    // P2035: Only = and <> on references
    #[test]
    fn compare_when_equality_on_reference_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    x : INT;
    r1 : REF_TO INT := REF(x);
    r2 : REF_TO INT := REF(x);
    result : BOOL;
END_VAR
    result := r1 = r2;
END_PROGRAM",
        );
    }

    #[test]
    fn compare_when_ordering_on_reference_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    x : INT;
    r1 : REF_TO INT := REF(x);
    r2 : REF_TO INT := REF(x);
    result : BOOL;
END_VAR
    result := r1 > r2;
END_PROGRAM",
        );
    }

    // P2032: Reference type mismatch
    #[test]
    fn assign_when_ref_types_match_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    x : INT;
    r : REF_TO INT;
END_VAR
    r := REF(x);
END_PROGRAM",
        );
    }

    #[test]
    fn assign_when_ref_types_incompatible_then_error() {
        assert_err(
            "PROGRAM Main
VAR
    x : REAL;
    r : REF_TO INT;
END_VAR
    r := REF(x);
END_PROGRAM",
        );
    }

    #[test]
    fn array_of_ref_to_when_declared_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    data : ARRAY[0..3] OF REF_TO BYTE;
END_VAR
END_PROGRAM",
        );
    }

    #[test]
    fn ref_to_array_when_declared_then_ok() {
        assert_ok(
            "PROGRAM Main
VAR
    data : REF_TO ARRAY[1..10] OF INT;
END_VAR
END_PROGRAM",
        );
    }

    #[test]
    fn ref_to_array_type_decl_when_declared_then_ok() {
        assert_ok(
            "TYPE ArrRef : REF_TO ARRAY[0..3] OF BYTE; END_TYPE
PROGRAM Main
VAR
    data : ArrRef;
END_VAR
END_PROGRAM",
        );
    }
}
