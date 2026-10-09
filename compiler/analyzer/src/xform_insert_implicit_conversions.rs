//! Lowering pass recording, in the AST, how each operand of a comparison is
//! converted before it is compared.
//!
//! A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=` and the functions `EQ`,
//! `NE`, `LT`, `LE`, `GT`, `GE`) compares its operands at one operand type.
//! This pass settles that type and makes it visible on the operands:
//!
//! * an operand of another scalar type is wrapped in an
//!   [`ExprKind::ImplicitConversion`] to the operand type, so `d < l` on a
//!   `DINT` and an `LINT` becomes `LINT(d) < l`;
//! * an untyped literal operand is given the operand type, so the `1` of
//!   `l > 1` is an `LINT`.
//!
//! After this pass both operands of a comparison have the operand type
//! wherever either has a type, so a backend compiles the comparison at the
//! type of its left operand and compiles a conversion where the node says,
//! without choosing either itself. See ADR-0056,
//! `specs/design/implicit-conversions.md` and
//! `specs/design/comparison-operand-type.md`.
//!
//! The pass runs in `stages::analyze` after the semantic rules, so a rule
//! checks the operands the program wrote. `rule_constant_range` runs after
//! the pass, checks each literal against the type the pass records for it,
//! and reads an operand through the conversion that wraps it. The pass
//! reports nothing: a comparison it cannot settle is left as it is.

mod argument;
mod arithmetic;
mod assignment;
mod declared;
mod inputs_of_one_type;
mod integer_result;
mod literal;

use std::convert::Infallible;

use ironplc_dsl::common::Library;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::{
    Assignment, Case, CompareExpr, Expr, ExprKind, FbCall, For, Function, If, ParamAssignmentKind,
    Repeat, StmtKind, While,
};
use ironplc_dsl::type_id::TypeId;
use ironplc_parser::options::CompilerOptions;

use crate::intermediates::comparison_operand::comparison_operand_type;
use crate::intermediates::conversion_target::{concrete, ConversionTarget};
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::semantic_context::SemanticContext;
use crate::symbol_environment::ScopeTracker;

pub fn apply(lib: Library, context: &SemanticContext, options: &CompilerOptions) -> Library {
    let declarations = declared::Declarations::collect(&lib, context.types());
    let mut inserter = ImplicitConversions {
        conversions: ConversionTarget::new(context.types()),
        declarations,
        context,
        scope: ScopeTracker::default(),
        options,
    };
    let Ok(lib) = inserter.fold_library(lib);
    lib
}

struct ImplicitConversions<'a> {
    conversions: ConversionTarget<'a>,
    /// The declared types of the fields and method parameters of every
    /// user-defined function block, and the order of its inputs.
    declarations: declared::Declarations,
    context: &'a SemanticContext,
    /// Where the traversal is, to look an assignment's target up in the
    /// symbol environment.
    scope: ScopeTracker,
    options: &'a CompilerOptions,
}

impl ImplicitConversions<'_> {
    /// Converts the operands of one comparison to its operand type.
    fn convert_operands(&self, left: &mut Expr, right: &mut Expr) {
        // A string is compared through the data region, in its own encoding;
        // there is nothing to convert it to.
        if self.conversions.is_string(left) || self.conversions.is_string(right) {
            return;
        }
        let Some(target) = self.operand_type(left, right) else {
            return;
        };
        self.convert(left, target);
        self.convert(right, target);
    }

    /// The type a comparison of `left` and `right` compares at: the type one
    /// operand widens to, else the concrete left operand's type, else the
    /// concrete right one's. The fallback is the rule codegen applied before
    /// the analyzer chose; a pair neither of which widens to the other
    /// (`DINT` and `UDINT`) is not checked yet (#1931).
    fn operand_type(&self, left: &Expr, right: &Expr) -> Option<TypeId> {
        comparison_operand_type(
            self.conversions.operand_name(left).as_ref(),
            self.conversions.operand_name(right).as_ref(),
            self.options,
        )
        .and_then(|common| self.conversions.id_of(&common))
        .or_else(|| concrete(left))
        .or_else(|| concrete(right))
    }
}

/// Returns `true` when `function` is a call to the function form of a
/// comparison, `EQ` to `GE`.
fn is_comparison_form(function: &Function) -> bool {
    operator_function_form(function.name.original())
        .is_some_and(|form| matches!(&form.operator, FormOf::Compare(op) if op.is_comparison()))
}

impl Fold<Infallible> for ImplicitConversions<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.record_assignment_value(&mut node);
        self.type_assignment_literals(&mut node.target, node.deref, &mut node.value);
        Ok(node)
    }

    fn fold_if(&mut self, node: If) -> Result<If, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.type_if_literals(&mut node);
        Ok(node)
    }

    fn fold_while(&mut self, node: While) -> Result<While, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.type_while_literals(&mut node);
        Ok(node)
    }

    fn fold_repeat(&mut self, node: Repeat) -> Result<Repeat, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.type_repeat_literals(&mut node);
        Ok(node)
    }

    fn fold_case(&mut self, node: Case) -> Result<Case, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.type_case_literals(&mut node);
        Ok(node)
    }

    fn fold_stmt_kind(&mut self, node: StmtKind) -> Result<StmtKind, Infallible> {
        let mut node = node.recurse_fold(self)?;
        if let StmtKind::MethodCall(call) = &mut node {
            self.record_method_arguments(call);
            self.type_method_call_statement_literals(call);
        }
        Ok(node)
    }

    fn fold_for(&mut self, node: For) -> Result<For, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.record_for_bounds(&mut node);
        self.type_for_literals(&mut node);
        Ok(node)
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.record_fb_call_inputs(&mut node);
        self.type_fb_call_literals(&mut node);
        Ok(node)
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Infallible> {
        let mut node = node.recurse_fold(self)?;
        match &mut node.kind {
            ExprKind::BinaryOp(binary) => {
                self.record_binary_operands(binary, node.expr_type.as_ref());
            }
            ExprKind::Function(_) => {
                self.record_fold_operands(&mut node);
                self.record_one_type_inputs(&mut node);
            }
            ExprKind::MethodCall(call) => self.record_method_arguments(call),
            // A comparison's operands are recorded by `fold_compare_expr`;
            // the operands of `AND`, `OR` and `XOR` here, since they are
            // converted to the operation's own type.
            ExprKind::Compare(_) => self.record_bitwise_operands(&mut node),
            // A function's arguments are recorded by `fold_function`;
            // nothing else has operands or arguments a context converts.
            ExprKind::UnaryOp(_)
            | ExprKind::Expression(_)
            | ExprKind::Const(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Variable(_)
            | ExprKind::LateBound(_)
            | ExprKind::Ref(_)
            | ExprKind::Deref(_)
            | ExprKind::ImplicitConversion(_)
            | ExprKind::Null(_) => {}
        }
        Ok(node)
    }

    fn fold_compare_expr(&mut self, node: CompareExpr) -> Result<CompareExpr, Infallible> {
        let mut node = node.recurse_fold(self)?;
        if node.op.is_comparison() {
            self.convert_operands(&mut node.left, &mut node.right);
        }
        Ok(node)
    }

    fn fold_function(&mut self, node: Function) -> Result<Function, Infallible> {
        let mut node = node.recurse_fold(self)?;
        self.record_argument_conversions(&mut node);
        if is_comparison_form(&node) {
            // The comparison forms are binary, and the named-argument pass
            // made every input positional; any other shape is one a rule
            // reported.
            if let [ParamAssignmentKind::PositionalInput(left), ParamAssignmentKind::PositionalInput(right)] =
                node.param_assignment.as_mut_slice()
            {
                self.convert_operands(&mut left.expr, &mut right.expr);
            }
        }
        Ok(node)
    }
}

#[cfg(test)]
mod stored_tests;
#[cfg(test)]
mod tests;
