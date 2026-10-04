//! Semantic rules for REF_TO reference types.
//!
//! This module validates the usage of REF_TO, REF(), NULL, and the dereference
//! operator (^) according to IEC 61131-3 Edition 3 safety constraints.

use ironplc_dsl::type_id::TypeId;
use ironplc_dsl::{
    common::*,
    core::{Id, Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use ironplc_parser::options::CompilerOptions;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::{ScopeTracker, SymbolEnvironment, SymbolInfo},
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleRefTo {
            type_environment: context.types(),
            symbols: context.symbols(),
            scope: ScopeTracker::default(),
            pou_kind: PouKind::Program,
            allow_ref_arithmetic: options.allow_ref_arithmetic,
            diagnostics: Vec::new(),
            allow_ref_stack_variables: options.allow_ref_stack_variables,
            allow_ref_type_punning: options.allow_ref_type_punning,
        },
        lib,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum PouKind {
    Function,
    FunctionBlock,
    Program,
}

struct RuleRefTo<'a> {
    type_environment: &'a TypeEnvironment,
    symbols: &'a SymbolEnvironment,
    /// Where the traversal is, to look variables up in the symbol
    /// environment. A method's scope nests inside its function block's,
    /// so a method body sees its own variables as well as the instance's
    /// fields.
    scope: ScopeTracker,
    /// The kind of POU currently being visited.
    pou_kind: PouKind,
    /// When true, allow arithmetic and ordering comparisons on REF_TO types.
    allow_ref_arithmetic: bool,
    diagnostics: Vec<Diagnostic>,
    /// When true, suppress P2029 (REF of stack-allocated variables).
    allow_ref_stack_variables: bool,
    /// When true, suppress P2032 type mismatch for REF_TO type punning.
    allow_ref_type_punning: bool,
}

impl DiagnosticVisitor for RuleRefTo<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Extracts a span from a Variable, falling back to default.
fn variable_span(var: &Variable) -> SourceSpan {
    match var {
        Variable::Symbolic(sym) => sym.span(),
        Variable::Direct(_) => SourceSpan::default(),
    }
}

impl RuleRefTo<'_> {
    /// Returns the symbol of the variable `name` names from the current
    /// scope.
    fn symbol(&self, name: &Id) -> Option<&SymbolInfo> {
        self.symbols.find(name, &self.scope.current())
    }

    /// Returns the id of a variable's declared type, if it has a name the
    /// type can be compared by.
    ///
    /// A field access (`s.f`) answers for its record. A type spelled out in
    /// place (an inline array) has no name, and each declaration of one is
    /// a type of its own, so it is not compared.
    fn variable_type_id(&self, var: &Variable) -> Option<TypeId> {
        let id = match var {
            Variable::Symbolic(SymbolicVariableKind::Named(named)) => &named.name,
            Variable::Symbolic(SymbolicVariableKind::Structured(s)) => match s.record.as_ref() {
                SymbolicVariableKind::Named(named) => &named.name,
                _ => return None,
            },
            _ => return None,
        };
        let type_id = self.symbol(id)?.type_id?;
        self.type_environment.name_of(type_id)?;
        Some(type_id)
    }

    /// Returns true if the given type resolves to a reference type.
    fn is_reference_type(&self, type_name: &TypeName) -> bool {
        self.type_environment
            .get(type_name)
            .map(|attrs| attrs.representation.is_reference())
            .unwrap_or(false)
    }

    /// Returns the id of the type of the value `kind` names: the declared
    /// type of a variable, and the element type of an array element.
    ///
    /// A field access has no answer: a field's type is known by its
    /// representation only, so it has no id to give.
    fn value_type_id(&self, kind: &SymbolicVariableKind) -> Option<TypeId> {
        match kind {
            SymbolicVariableKind::Named(named) => self.symbol(&named.name)?.type_id,
            SymbolicVariableKind::Array(array) => self
                .type_environment
                .element_type(self.value_type_id(&array.subscripted_variable)?),
            _ => None,
        }
    }

    /// Returns the id of the type of the value `var` names, when it is a
    /// reference type: a reference variable, or an element of an array of
    /// references.
    fn reference_type_id(&self, var: &Variable) -> Option<TypeId> {
        let Variable::Symbolic(kind) = var else {
            return None;
        };
        let type_id = self.value_type_id(kind)?;
        self.type_environment
            .get_by_id(type_id)?
            .representation
            .is_reference()
            .then_some(type_id)
    }

    /// Returns true if the value the variable names is a reference.
    fn is_variable_reference(&self, var: &Variable) -> bool {
        self.reference_type_id(var).is_some()
    }

    /// Returns true if the expression resolves to a reference type.
    fn is_expr_reference(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ref(_) => true,
            ExprKind::Null(_) => true,
            ExprKind::Variable(var) => self.is_variable_reference(var),
            // Any other expression is a reference when its value's type is.
            ExprKind::Compare(_)
            | ExprKind::BinaryOp(_)
            | ExprKind::UnaryOp(_)
            | ExprKind::Expression(_)
            | ExprKind::Const(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Function(_)
            | ExprKind::MethodCall(_)
            | ExprKind::LateBound(_)
            | ExprKind::Deref(_)
            | ExprKind::ImplicitConversion(_) => matches!(
                self.type_environment.representation_of_expr(expr),
                Some(crate::semantic_type::SemanticType::Reference { .. })
            ),
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

        // P2029: Check for ephemeral variables.
        // Suppressed when allow_ref_stack_variables is enabled — OSCAT relies
        // on REF() of function parameters for type punning patterns where the
        // reference never escapes the function.
        if !self.allow_ref_stack_variables {
            if let Variable::Symbolic(SymbolicVariableKind::Named(named)) = var {
                if let Some(var_type) = self
                    .symbol(&named.name)
                    .and_then(|info| info.variable_type.clone())
                {
                    match var_type {
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
        if self.allow_ref_arithmetic {
            return;
        }
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
                // Equality and inequality are always allowed on references
            }
            CompareOp::Lt | CompareOp::Gt | CompareOp::LtEq | CompareOp::GtEq => {
                if self.allow_ref_arithmetic {
                    return;
                }
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
            CompareOp::Or
            | CompareOp::Xor
            | CompareOp::And
            | CompareOp::AndThen
            | CompareOp::OrElse => {}
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
            } else if !self.allow_ref_type_punning {
                // Check type compatibility (P2032 type mismatch branch).
                // Suppressed when allow_ref_type_punning is enabled — OSCAT
                // uses REF() to reinterpret a REAL's bits as DWORD.
                let target_ref_type = self.get_reference_target_type(target);
                let operand_type = self.variable_type_id(ref_var);
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

    /// Returns the id of the type a REF_TO variable references, when that
    /// type has a name the type can be compared by.
    fn get_reference_target_type(&self, var: &Variable) -> Option<TypeId> {
        let target = self
            .type_environment
            .referenced_type(self.reference_type_id(var)?)?;
        self.type_environment.name_of(target)?;
        Some(target)
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

impl Visitor<Infallible> for RuleRefTo<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Infallible> {
        self.pou_kind = PouKind::Function;
        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Infallible> {
        self.pou_kind = PouKind::FunctionBlock;
        node.recurse_visit(self)
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Infallible> {
        self.pou_kind = PouKind::Program;
        node.recurse_visit(self)
    }

    fn visit_reference_declaration(
        &mut self,
        node: &ReferenceDeclaration,
    ) -> Result<(), Infallible> {
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

    fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
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

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.check_null_assignment(&node.target, &node.value);
        self.check_ref_assignment(&node.target, &node.value);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests;
