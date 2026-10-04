//! Recording the type of an untyped literal from the context it is used in.
//!
//! An untyped literal such as `1` has a generic type (`ANY_INT`) until a
//! context gives it one (ADR-0028). The code generator gives it one top-down:
//! a statement passes the type it stores or compares at into the expression,
//! and that type flows through a negation, parentheses and an arithmetic
//! operation whose own type is generic, until it reaches the literal. This
//! module records the type each literal reaches, so that a backend reads it
//! from the literal rather than carrying it down.
//!
//! It covers the statement contexts: the value of an assignment, the bounds
//! and step of a `FOR` loop, and the inputs of a function block call. A
//! literal inside a standard function call or a comparison is typed by its
//! own construct.

use ironplc_dsl::common::{ElementaryTypeName, TypeName};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{
    Expr, ExprKind, ExprType, FbCall, For, ParamAssignmentKind, SymbolicVariableKind, Variable,
};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::stdlib_function_block::is_stdlib_function_block;
use crate::semantic_type::SemanticType;
use crate::variable_type;

impl ImplicitConversions<'_> {
    /// Gives the untyped literals of `expr` the type `context`, where the
    /// code generator compiles them at it.
    pub(super) fn type_literals(&self, expr: &mut Expr, context: TypeId) {
        let context = match expr.expr_type {
            Some(ExprType::Literal(_)) => {
                expr.expr_type = Some(ExprType::Concrete(context));
                context
            }
            _ => context,
        };
        match &mut expr.kind {
            ExprKind::UnaryOp(unary) => self.type_literals(&mut unary.term, context),
            ExprKind::Expression(inner) => self.type_literals(inner, context),
            // An arithmetic operation computes at its own type; the
            // arithmetic pass converted its operands to it.
            ExprKind::BinaryOp(binary) => {
                if let Some(ExprType::Concrete(own)) = expr.expr_type {
                    self.type_literals(&mut binary.left, own);
                    self.type_literals(&mut binary.right, own);
                }
            }
            // A conversion compiles its operand at the operand's own type.
            ExprKind::ImplicitConversion(inner) => {
                if let Some(ExprType::Concrete(own)) = inner.expr_type {
                    self.type_literals(inner, own);
                }
            }
            _ => {}
        }
    }

    /// The type the value assigned to `target` is compiled at, or `None` for
    /// a target this pass does not type yet (a directly represented
    /// variable) or that is not a single value (a string, an aggregate).
    pub(super) fn assigned_at(&self, target: &Variable, deref: bool) -> Option<TypeId> {
        // A dereference stores at the default slot type.
        if deref {
            return self.dint();
        }
        let Variable::Symbolic(kind) = target else {
            return None;
        };
        if let SymbolicVariableKind::Structured(structured) = kind {
            if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
                if let Some(at) = self.function_block_field_at(&named.name, &structured.field) {
                    return at;
                }
            }
        }
        self.stored_as(target).map(|(id, _)| id)
    }

    /// The type an input of the function block instance `instance` named
    /// `field` is stored at: a field of a user-defined block at its declared
    /// type, and one of a standard block at the default slot type. `None`
    /// when `instance` is not a function block instance.
    fn function_block_field_at(&self, instance: &Id, field: &Id) -> Option<Option<TypeId>> {
        let representation =
            variable_type::declared(instance, self.context, &self.scope.current())?;
        let SemanticType::FunctionBlock { name, fields } = representation else {
            return None;
        };
        if is_stdlib_function_block(&Id::from(name.as_str())) {
            return Some(self.dint());
        }
        let field_type = fields
            .iter()
            .find(|candidate| candidate.name == *field)
            .map(|candidate| &candidate.field_type);
        Some(field_type.and_then(|field_type| self.elementary_of(field_type)))
    }

    /// Types the literals of the bounds and step of `node` by its control
    /// variable.
    pub(super) fn type_for_literals(&self, node: &mut For) {
        let control = variable_type::declared(&node.control, self.context, &self.scope.current())
            .and_then(|representation| self.elementary_of(representation))
            .or_else(|| self.dint());
        let Some(control) = control else {
            return;
        };
        self.type_literals(&mut node.from, control);
        self.type_literals(&mut node.to, control);
        if let Some(step) = &mut node.step {
            self.type_literals(step, control);
        }
    }

    /// Types the literals of the named inputs of `node` by the fields they
    /// are stored in.
    pub(super) fn type_fb_call_literals(&self, node: &mut FbCall) {
        for param in &mut node.params {
            if let ParamAssignmentKind::NamedInput(input) = param {
                if let Some(Some(at)) = self.function_block_field_at(&node.var_name, &input.name) {
                    self.type_literals(&mut input.expr, at);
                }
            }
        }
    }

    /// The elementary type a value of `representation` is operated as: its
    /// own, or its base type for a subrange.
    fn elementary_of(&self, representation: &SemanticType) -> Option<TypeId> {
        let representation = match representation {
            SemanticType::Subrange { base_type, .. } => base_type,
            other => other,
        };
        let types = self.context.types();
        types.id_of(&types.elementary_type_name_for(representation)?)
    }

    /// The default slot type, `DINT`.
    fn dint(&self) -> Option<TypeId> {
        let dint: TypeName = ElementaryTypeName::DINT.into();
        self.context.types().id_of(&dint)
    }
}
