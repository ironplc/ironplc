//! Addressing a lowered place in the storage codegen lays out.
//!
//! While both routes exist, a lowered place is stored where codegen's own
//! layout puts what the place names, and that layout is found by name,
//! through [`Place::resolve`] on a variable reference. So a lowered place is
//! addressed by rebuilding the variable reference it stands for, with a
//! numbered placeholder for each subscript, resolving that, and putting the
//! lowered subscripts in the placeholders' places. Both routes then address
//! an element through the same code, bounds check included.

use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    ArrayVariable, Expr as AstExpr, ExprKind as AstExprKind, NamedVariable, StructuredVariable,
    SymbolicVariableKind,
};
use ironplc_ir::lowered::{Const, Expr, ExprKind, Place as LoweredPlace, Projection, TypeKind};
use ironplc_lowering::Lowering;

use super::expr::emit_expr;
use crate::compile::CompileContext;
use crate::compile_place::Place;
use crate::compile_subscript::Subscript;
use crate::emit::Emitter;

/// A subscript of a lowered place.
pub(super) struct LoweredSubscript<'l, 'a> {
    lowering: &'l Lowering<'a>,
    expr: &'l Expr,
}

impl Subscript for LoweredSubscript<'_, '_> {
    fn literal(&self) -> Option<i32> {
        match self.expr.kind() {
            ExprKind::Const(Const::I32(value)) => Some(*value),
            _ => None,
        }
    }

    fn emit(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
        emit_expr(emitter, ctx, self.lowering, self.expr)
    }
}

/// Where codegen's layout stores `place`, as one value in one slot.
pub(super) fn resolve<'l, 'a>(
    lowering: &'l Lowering<'a>,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<Place<LoweredSubscript<'l, 'a>>, Diagnostic> {
    let no_storage = || {
        Diagnostic::internal_error_at(Label::span(span.clone(), "A lowered place has no storage"))
    };
    let mut subscripts = Vec::new();
    let reference = reference(lowering, place, &mut subscripts).ok_or_else(no_storage)?;
    Place::resolve(ctx, &reference)?.try_map_subscripts(|placeholder| {
        let expr = placeholder
            .literal()
            .and_then(|position| subscripts.get(usize::try_from(position).ok()?))
            .ok_or_else(no_storage)?;
        Ok(LoweredSubscript { lowering, expr })
    })
}

/// The variable reference `place` stands for, with the subscript at
/// position `k` of `subscripts` as the literal `k`; `None` for a place this
/// backend does not reach yet.
fn reference<'l>(
    lowering: &Lowering<'_>,
    place: &'l LoweredPlace,
    subscripts: &mut Vec<&'l Expr>,
) -> Option<SymbolicVariableKind> {
    let types = lowering.types();
    let root = lowering.variables().get(place.root())?;
    let mut ty = root.ty();
    let mut path = place.path();
    let mut reference = match path {
        // A field of the current instance, which codegen copies into a slot
        // of the field's name.
        [Projection::Deref, Projection::Field(field), rest @ ..] => {
            let (instance, _) = types.reference(ty)?;
            let declared = types.fields(instance)?.get(field.index() as usize)?;
            ty = declared.ty;
            path = rest;
            named(&declared.name.to_string())
        }
        _ => named(&root.name().to_string()),
    };
    for projection in path {
        reference = match projection {
            Projection::Field(field) => {
                let declared = types.fields(ty)?.get(field.index() as usize)?;
                ty = declared.ty;
                SymbolicVariableKind::Structured(StructuredVariable {
                    record: Box::new(reference),
                    field: Id::from(&declared.name.to_string()),
                })
            }
            Projection::Index(exprs) => {
                let Some(TypeKind::Array { element, .. }) = types.kind(ty) else {
                    return None;
                };
                ty = *element;
                let placeholders = exprs
                    .iter()
                    .map(|expr| {
                        let position = subscripts.len();
                        subscripts.push(expr);
                        AstExpr::new(AstExprKind::integer_literal(&position.to_string()))
                    })
                    .collect();
                SymbolicVariableKind::Array(ArrayVariable {
                    subscripted_variable: Box::new(reference),
                    subscripts: placeholders,
                })
            }
            Projection::Deref => return None,
        };
    }
    Some(reference)
}

fn named(name: &str) -> SymbolicVariableKind {
    SymbolicVariableKind::Named(NamedVariable {
        name: Id::from(name),
    })
}
