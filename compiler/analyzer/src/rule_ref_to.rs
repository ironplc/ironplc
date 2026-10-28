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
    variable_type,
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
        self.symbols
            .resolve_variable(name, &self.scope.current())
            .declared()
    }

    /// Returns true if the given type resolves to a reference type.
    fn is_reference_type(&self, type_name: &TypeName) -> bool {
        self.type_environment
            .get(type_name)
            .map(|attrs| attrs.representation.is_reference())
            .unwrap_or(false)
    }

    /// Returns true when the type `id` identifies is a reference type.
    fn is_reference_id(&self, id: TypeId) -> bool {
        self.type_environment
            .get_by_id(id)
            .is_some_and(|attributes| attributes.representation.is_reference())
    }

    /// The id of the type of the value `var` names: its declared type,
    /// walked through fields, subscripts and dereferences by the
    /// environment (`h.rr` is the field's type, `ra[0]` the element's, `pp^`
    /// what `pp` refers to). `None` for a directly represented variable and
    /// for a name the walk cannot type.
    fn variable_type_id(&self, var: &Variable) -> Option<TypeId> {
        let Variable::Symbolic(kind) = var else {
            return None;
        };
        variable_type::type_id(
            kind,
            self.symbols,
            self.type_environment,
            &self.scope.current(),
        )
    }

    /// The id of the type of the value of `expr`: a variable by the walk
    /// above, any other expression by the type the analyzer resolved for
    /// it. `None` for an untyped literal, `NULL`, and an expression the
    /// analyzer did not type.
    fn value_type_id(&self, expr: &Expr) -> Option<TypeId> {
        match &expr.kind {
            ExprKind::Variable(var) => self.variable_type_id(var),
            ExprKind::Expression(inner) => self.value_type_id(inner),
            _ => match expr.expr_type.as_ref()? {
                ExprType::Concrete(id) | ExprType::Inferred(id) => Some(*id),
                ExprType::Literal(_) | ExprType::Null => None,
            },
        }
    }

    /// The id of the type of the reference `expr` names, when `expr` is a
    /// place that holds one: a variable (`ri`, `refs[0]`, `h.r`) or what a
    /// reference refers to (`pp^`). `None` for any other expression, and
    /// for a place that holds a value.
    fn held_reference_type_id(&self, expr: &Expr) -> Option<TypeId> {
        let id = match &expr.kind {
            ExprKind::Variable(var) => self.variable_type_id(var)?,
            ExprKind::Deref(_) => self.value_type_id(expr)?,
            ExprKind::Expression(inner) => return self.held_reference_type_id(inner),
            _ => return None,
        };
        self.is_reference_id(id).then_some(id)
    }

    /// The id of the type of the place an assignment stores to: its
    /// target's type, or for a store through a reference (`p^ := v`) the
    /// type `p` refers to.
    fn assigned_type_id(&self, node: &Assignment) -> Option<TypeId> {
        let target = self.variable_type_id(&node.target)?;
        if node.deref {
            self.type_environment.referenced_type(target)
        } else {
            Some(target)
        }
    }

    /// Returns true if the value the variable names is a reference.
    fn is_variable_reference(&self, var: &Variable) -> bool {
        self.variable_type_id(var)
            .is_some_and(|id| self.is_reference_id(id))
    }

    /// Returns true if the expression resolves to a reference type.
    fn is_expr_reference(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ref(_) | ExprKind::Null(_) => true,
            // Any other expression is a reference when its value's type is.
            _ => self
                .value_type_id(expr)
                .is_some_and(|id| self.is_reference_id(id)),
        }
    }

    /// Whether the types `a` and `b` identify are the same type, when that
    /// can be said. A reference type is one type however it is spelled or
    /// named, so two reference types are the same when what they refer to
    /// is; a reference and a value never are. Any other type is itself
    /// only, compared by id when both have a name: a type spelled out in
    /// place (an inline array) is a type of its own at each declaration, so
    /// it is not compared, and the answer is `None`.
    fn same_type(&self, a: TypeId, b: TypeId) -> Option<bool> {
        if a == b {
            return Some(true);
        }
        let (a_reference, b_reference) = (self.is_reference_id(a), self.is_reference_id(b));
        if a_reference != b_reference {
            return Some(false);
        }
        if a_reference {
            let referenced = |id| self.type_environment.referenced_type(id);
            return self.same_type(referenced(a)?, referenced(b)?);
        }
        let named = |id| self.type_environment.name_of(id).is_some();
        (named(a) && named(b)).then_some(false)
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

    /// P2031: Dereference requires reference type. A variable operand is
    /// judged by the environment's walk of it; any other operand (`(r^)^`,
    /// `f()^`, `5^`) by the type the analyzer resolved for it, and one whose
    /// type is not resolved is not reported. A dereference of a dereference
    /// (`pp^^`) is a reference when `pp^` is one.
    fn check_deref(&mut self, inner: &Expr) {
        let is_reference = match &inner.kind {
            ExprKind::Variable(var) => self.is_variable_reference(var),
            _ => !self.is_known_non_reference(inner),
        };
        if is_reference {
            return;
        }
        self.diagnostics.push(Diagnostic::problem(
            Problem::DerefRequiresReferenceType,
            Label::span(
                inner.span(),
                "Dereference operator (^) requires a REF_TO type",
            ),
        ));
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

    /// P4049: A reference compared with a value that is not a reference
    /// (`rk = 0`, `rk = k`, `k = NULL`). P2035: Only = and <> on references
    fn check_compare_op(&mut self, expr: &CompareExpr) {
        let left_ref = self.is_expr_reference(&expr.left);
        let right_ref = self.is_expr_reference(&expr.right);
        if !left_ref && !right_ref {
            return;
        }
        if self.check_compared_with_reference(expr, left_ref) {
            return;
        }
        match expr.op.kind() {
            CompareOpKind::Equality => {
                // Equality and inequality are always allowed on references
            }
            CompareOpKind::Ordering => {
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
            CompareOpKind::Bitwise => {}
        }
    }

    /// Reports P4049 when `expr` compares a reference with a value that is
    /// known not to be one, and returns whether it did. `left_ref` is
    /// whether the left operand is the reference. A reference equals
    /// `NULL` or another reference, never a number: a CODESYS pointer
    /// compared with 0 is reported too.
    fn check_compared_with_reference(&mut self, expr: &CompareExpr, left_ref: bool) -> bool {
        if !expr.op.is_comparison() {
            return false;
        }
        let other = if left_ref { &expr.right } else { &expr.left };
        if !self.is_known_non_reference(other) {
            return false;
        }
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::OperatorOperandTypeMismatch,
                Label::span(other.span(), "Operand that is not a reference"),
            )
            .with_context("operator", &expr.op.to_string())
            .with_context("expected", &"a reference or NULL".to_string()),
        );
        true
    }

    /// Returns true when `expr` is known not to be a reference: a literal,
    /// or an expression whose type the analyzer resolved to a type that is
    /// not a reference. Where reference arithmetic is allowed, a sum or
    /// difference with a reference operand is a reference (`p + 1`).
    fn is_known_non_reference(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Const(_) => true,
            ExprKind::Ref(_) | ExprKind::Null(_) => false,
            ExprKind::Expression(inner) => self.is_known_non_reference(inner),
            ExprKind::BinaryOp(binary)
                if self.allow_ref_arithmetic
                    && (self.is_expr_reference(&binary.left)
                        || self.is_expr_reference(&binary.right)) =>
            {
                false
            }
            ExprKind::Compare(_)
            | ExprKind::BinaryOp(_)
            | ExprKind::UnaryOp(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Variable(_)
            | ExprKind::Function(_)
            | ExprKind::MethodCall(_)
            | ExprKind::LateBound(_)
            | ExprKind::Deref(_)
            | ExprKind::ImplicitConversion(_)
            | ExprKind::CallOutput(_) => self
                .value_type_id(expr)
                .is_some_and(|id| !self.is_reference_id(id)),
        }
    }

    /// P2032: A value that is not a reference assigned to a reference: a
    /// literal (`rk := 0`), or a variable or an expression of another type
    /// (`rk := k`), to a reference variable or through a reference to one
    /// (`pp^ := 0` on a `REF_TO REF_TO INT`). A reference takes `REF()`,
    /// `NULL` or another reference. A store through a reference to a value
    /// (`rk^ := 0`, and a bare store to a `REFERENCE TO`, which the analyzer
    /// spells as one) stores the value itself.
    fn check_value_assigned_to_reference(&mut self, node: &Assignment) {
        if node.ref_bind || node.set_bind || node.reset_bind {
            return;
        }
        let Some(assigned) = self.assigned_type_id(node) else {
            return;
        };
        if !self.is_reference_id(assigned) || !self.is_known_non_reference(&node.value) {
            return;
        }
        self.diagnostics.push(Diagnostic::problem(
            Problem::ReferenceTypeMismatch,
            Label::span(
                node.value.span(),
                "Value assigned to a reference is not a reference",
            ),
        ));
    }

    /// P2034: NULL can only be assigned to a reference: a reference
    /// variable, or what a reference to a reference refers to (`pp^`).
    /// `rk^ := NULL` on a `REF_TO INT` stores into the `INT`.
    fn check_null_assignment(&mut self, node: &Assignment) {
        let ExprKind::Null(span) = &node.value.kind else {
            return;
        };
        let Some(assigned) = self.assigned_type_id(node) else {
            return;
        };
        if !self.is_reference_id(assigned) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::NullRequiresReferenceType,
                Label::span(span.clone(), "NULL can only be assigned to a REF_TO type"),
            ));
        }
    }

    /// P2032: `REF(x)` assigned where it does not fit: to a place that is
    /// not a reference, or to a reference to another type than `x`'s
    /// (`ri := REF(d)`, and `ri := REF(ri)`, a reference to a reference
    /// stored in a reference to a value), unless type punning is allowed.
    fn check_ref_assignment(&mut self, node: &Assignment) {
        let ExprKind::Ref(ref_var) = &node.value.kind else {
            return;
        };
        if node.set_bind || node.reset_bind {
            return;
        }
        let ref_span = variable_span(ref_var);
        let Some(assigned) = self.assigned_type_id(node) else {
            return;
        };
        if !self.is_reference_id(assigned) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ReferenceTypeMismatch,
                Label::span(ref_span, "Reference type mismatch in assignment"),
            ));
            return;
        }
        // Suppressed when allow_ref_type_punning is enabled — OSCAT uses
        // REF() to reinterpret a REAL's bits as DWORD.
        if self.allow_ref_type_punning {
            return;
        }
        let (Some(referenced), Some(operand)) = (
            self.type_environment.referenced_type(assigned),
            self.variable_type_id(ref_var),
        ) else {
            return;
        };
        if self.same_type(referenced, operand) == Some(false) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ReferenceTypeMismatch,
                Label::span(ref_span, "Reference type mismatch in assignment"),
            ));
        }
    }

    /// P2032: A reference held in a variable, or in what a reference refers
    /// to (`pp^`), assigned where it does not fit: to a place that is not a
    /// reference, which would store the variable index the reference holds
    /// (`x := ri`, `x := pp^`), or to a reference to another type (`ri :=
    /// rd`, `pp := ri` on a `REF_TO REF_TO INT`), unless type punning is
    /// allowed.
    ///
    /// A read through a reference to a value is a dereference (`ri^`),
    /// which the analyzer spells as one even for a bare `REFERENCE TO`
    /// read, so the value here is the reference itself. `REF()` and `NULL`
    /// are checked by the rules above, and an expression on a reference
    /// (`ri + 1`) by the arithmetic rule.
    fn check_reference_value_assignment(&mut self, node: &Assignment) {
        if node.ref_bind || node.set_bind || node.reset_bind {
            return;
        }
        let Some(value) = self.held_reference_type_id(&node.value) else {
            return;
        };
        let span = node.value.span();
        let Some(assigned) = self.assigned_type_id(node) else {
            return;
        };
        if !self.is_reference_id(assigned) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ReferenceTypeMismatch,
                Label::span(
                    span,
                    "Reference assigned to a variable that is not a reference",
                ),
            ));
            return;
        }
        if self.allow_ref_type_punning {
            return;
        }
        if self.same_type(assigned, value) == Some(false) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ReferenceTypeMismatch,
                Label::span(span, "Reference type mismatch in assignment"),
            ));
        }
    }

    /// P2036: A TwinCAT `REFERENCE TO` whose target is a reference type
    /// (`rr : REFERENCE TO IntRef`, `rp : REFERENCE TO POINTER TO INT`), or
    /// a reference of any syntax whose target is spelled `REFERENCE TO`
    /// (`POINTER TO REFERENCE TO INT`), labelled at `span`. TwinCAT declares
    /// neither `REFERENCE TO REFERENCE` nor `POINTER TO REFERENCE`. IronPLC
    /// gives a `REFERENCE TO` variable its auto-dereference by its own
    /// declaration (`xform_insert_implicit_deref`), which a reference
    /// reached through another has not, and which does not reach through a
    /// pointer the variable refers to, so a `REFERENCE TO` a pointer, which
    /// TwinCAT declares, is not supported either. `REF_TO REF_TO INT` and
    /// `POINTER TO POINTER TO INT` are nested references, which IEC 61131-3
    /// and CODESYS allow.
    fn check_reference_to_nested(
        &mut self,
        syntax: RefSyntax,
        target: &ReferenceTarget,
        span: SourceSpan,
    ) {
        if self.is_reference_to_nested(syntax, target) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::NestedRefToNotSupported,
                Label::span(span, "REFERENCE TO a reference is not supported"),
            ));
        }
    }

    /// Whether a reference spelled with `syntax` to `target` nests a
    /// `REFERENCE TO` at any level.
    fn is_reference_to_nested(&self, syntax: RefSyntax, target: &ReferenceTarget) -> bool {
        match target {
            ReferenceTarget::Named(name) => {
                syntax == RefSyntax::ReferenceTo && self.is_reference_type(name)
            }
            ReferenceTarget::Array(_) => false,
            ReferenceTarget::Reference(spec) => {
                syntax == RefSyntax::ReferenceTo
                    || spec.syntax == RefSyntax::ReferenceTo
                    || self.is_reference_to_nested(spec.syntax, &spec.target)
            }
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
        self.check_reference_to_nested(node.syntax, &node.target, node.type_name.span());
        node.recurse_visit(self)
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        if let InitialValueAssignmentKind::Reference(init) = &node.initializer {
            // Labelled at the named target, or at the variable for a target
            // spelled in place, whose elementary type name has no span.
            let span = match &init.target {
                ReferenceTarget::Named(name) => name.span(),
                ReferenceTarget::Array(_) | ReferenceTarget::Reference(_) => node.identifier.span(),
            };
            self.check_reference_to_nested(init.syntax, &init.target, span);
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
        self.check_null_assignment(node);
        self.check_ref_assignment(node);
        self.check_reference_value_assignment(node);
        self.check_value_assigned_to_reference(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod field_tests;

#[cfg(test)]
mod nested_reference_tests;

#[cfg(test)]
mod non_reference_value_tests;

#[cfg(test)]
mod reference_value_tests;

#[cfg(test)]
mod tests;
