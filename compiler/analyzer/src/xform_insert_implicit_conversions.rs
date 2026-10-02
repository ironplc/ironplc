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
//! checks the operands the program wrote. It reports nothing: a comparison
//! it cannot settle is left as it is.

use std::convert::Infallible;

use ironplc_dsl::common::Library;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::{CompareExpr, Expr, ExprKind, ExprType, Function, ParamAssignmentKind};
use ironplc_dsl::type_id::TypeId;
use ironplc_parser::options::CompilerOptions;

use crate::intermediate_type::IntermediateType;
use crate::intermediates::comparison_operand::comparison_operand_type;
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

pub fn apply(lib: Library, types: &TypeEnvironment, options: &CompilerOptions) -> Library {
    let mut inserter = ImplicitConversions { types, options };
    let Ok(lib) = inserter.fold_library(lib);
    lib
}

struct ImplicitConversions<'a> {
    types: &'a TypeEnvironment,
    options: &'a CompilerOptions,
}

impl ImplicitConversions<'_> {
    /// Converts the operands of one comparison to its operand type.
    fn convert_operands(&self, left: &mut Expr, right: &mut Expr) {
        // A string is compared through the data region, in its own encoding;
        // there is nothing to convert it to.
        if self.is_string(left) || self.is_string(right) {
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
            self.operand_name(left).as_ref(),
            self.operand_name(right).as_ref(),
            self.options,
        )
        .and_then(|common| self.types.id_of(&common))
        .or_else(|| concrete(left))
        .or_else(|| concrete(right))
    }

    /// Makes `operand` a value of the type `target`: an untyped literal is
    /// given the type, and a scalar operand of another type is wrapped
    /// in a conversion to it.
    fn convert(&self, operand: &mut Expr, target: TypeId) {
        match operand.expr_type {
            Some(ExprType::Literal(_)) => operand.expr_type = Some(ExprType::Concrete(target)),
            Some(ExprType::Concrete(own)) if self.needs_conversion(own, target) => {
                let placeholder = Expr::new(ExprKind::Null(operand.span.clone()));
                let inner = std::mem::replace(operand, placeholder);
                *operand = Expr::implicit_conversion(inner, target);
            }
            Some(ExprType::Concrete(_) | ExprType::Null) | None => {}
        }
    }

    /// Returns `true` when a value of type `own` is converted to be compared
    /// at `target`: both are scalars, and they are different types rather
    /// than one type under two names (an alias and the type it aliases, or
    /// an anonymous subrange and its base type).
    ///
    /// An enumeration or a reference is compared as the type it is.
    fn needs_conversion(&self, own: TypeId, target: TypeId) -> bool {
        own != target
            && self.is_scalar(own)
            && self.is_scalar(target)
            && self.name_of(own) != self.name_of(target)
    }

    /// Returns `true` for an elementary type, or a subrange of one.
    fn is_scalar(&self, id: TypeId) -> bool {
        self.representation(id).is_some_and(|representation| {
            representation.is_primitive() || representation.is_subrange()
        })
    }

    fn is_string(&self, expr: &Expr) -> bool {
        match &expr.expr_type {
            Some(ExprType::Concrete(id)) => matches!(
                self.representation(*id),
                Some(IntermediateType::String { .. })
            ),
            Some(ExprType::Literal(generic)) => {
                *generic == ironplc_dsl::common::GenericTypeName::AnyString
            }
            Some(ExprType::Null) | None => false,
        }
    }

    fn representation(&self, id: TypeId) -> Option<&IntermediateType> {
        self.types
            .get_by_id(id)
            .map(|attributes| &attributes.representation)
    }

    fn name_of(&self, id: TypeId) -> Option<ironplc_dsl::common::TypeName> {
        operand_type_name(self.types, &ExprType::Concrete(id))
    }

    fn operand_name(&self, expr: &Expr) -> Option<ironplc_dsl::common::TypeName> {
        operand_type_name(self.types, expr.expr_type.as_ref()?)
    }
}

/// The type of `expr` when it is a value of one concrete type.
fn concrete(expr: &Expr) -> Option<TypeId> {
    match expr.expr_type {
        Some(ExprType::Concrete(id)) => Some(id),
        Some(ExprType::Literal(_) | ExprType::Null) | None => None,
    }
}

/// Returns `true` when `function` is a call to the function form of a
/// comparison, `EQ` to `GE`.
fn is_comparison_form(function: &Function) -> bool {
    operator_function_form(function.name.original())
        .is_some_and(|form| matches!(&form.operator, FormOf::Compare(op) if op.is_comparison()))
}

impl Fold<Infallible> for ImplicitConversions<'_> {
    fn fold_compare_expr(&mut self, node: CompareExpr) -> Result<CompareExpr, Infallible> {
        let mut node = node.recurse_fold(self)?;
        if node.op.is_comparison() {
            self.convert_operands(&mut node.left, &mut node.right);
        }
        Ok(node)
    }

    fn fold_function(&mut self, node: Function) -> Result<Function, Infallible> {
        let mut node = node.recurse_fold(self)?;
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
mod tests;
