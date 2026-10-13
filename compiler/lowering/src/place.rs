//! Places named in the source: a variable, a field of a structure, an
//! element of an array, and what a reference refers to.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, ExprKind as AstExprKind, SymbolicVariableKind};
use ironplc_ir::lowered::{Expr, Place};

use crate::variables::BodyFrame;
use crate::Lowering;

/// A lowered node, `None` when lowering does not support it, or something
/// inside it, yet.
type Made<T> = Result<Option<T>, Diagnostic>;

impl Lowering<'_> {
    /// The place `symbolic` names in the body of `frame`.
    pub(crate) fn symbolic_place(
        &mut self,
        frame: &BodyFrame,
        symbolic: &SymbolicVariableKind,
    ) -> Made<Place> {
        match symbolic {
            SymbolicVariableKind::Named(named) => {
                self.named_place(frame, &named.name, &named.name.span())
            }
            SymbolicVariableKind::Structured(structured) => {
                let Some(record) = self.symbolic_place(frame, &structured.record)? else {
                    return Ok(None);
                };
                let Some(position) = self.program.types().fields(record.ty()).and_then(|fields| {
                    fields
                        .iter()
                        .position(|field| Id::from(&field.name.to_string()) == structured.field)
                }) else {
                    return Ok(None);
                };
                let Some(field) = self.program.field_idx(record.ty(), position) else {
                    return Ok(None);
                };
                self.program
                    .field(record, field, &structured.field.span())
                    .map(Some)
            }
            SymbolicVariableKind::Array(array) => {
                let Some(base) = self.symbolic_place(frame, &array.subscripted_variable)? else {
                    return Ok(None);
                };
                let mut subscripts = Vec::with_capacity(array.subscripts.len());
                for subscript in &array.subscripts {
                    let Some(subscript) = self.subscript(frame, subscript)? else {
                        return Ok(None);
                    };
                    subscripts.push(subscript);
                }
                self.program
                    .index(base, subscripts, &array.span())
                    .map(Some)
            }
            SymbolicVariableKind::Deref(deref) => {
                let Some(reference) = self.symbolic_place(frame, &deref.variable)? else {
                    return Ok(None);
                };
                self.program.deref(reference, &deref.span()).map(Some)
            }
            SymbolicVariableKind::BitAccess(_)
            | SymbolicVariableKind::PartialAccess(_)
            | SymbolicVariableKind::SelfRef(_) => Ok(None),
        }
    }

    /// A subscript, a `DINT` (REQ-LOW-lowering-161). The analyzer converts a
    /// subscript of another type to `DINT`. That conversion is exact for a
    /// type whose values all fit a `DINT`; for any other, such as a `UDINT`,
    /// it would wrap, so such a subscript is not lowered until the analyzer
    /// records its conversion as checked (REQ-LOW-analyzer-162).
    fn subscript(&mut self, frame: &BodyFrame, subscript: &AstExpr) -> Made<Expr> {
        if let AstExprKind::ImplicitConversion(converted) = &subscript.kind {
            if !self.fits_dint(converted)? {
                return Ok(None);
            }
        }
        let Some(value) = self.expr(frame, subscript)? else {
            return Ok(None);
        };
        if !value.ty().is_numeric() {
            // A `BOOL` subscript, which analysis does not reject yet. A number
            // of another type than `DINT` is a conversion the analyzer did not
            // record, a defect, which `Place::index` reports.
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// Whether every value of `expr`'s type fits a `DINT`.
    fn fits_dint(&mut self, expr: &AstExpr) -> Result<bool, Diagnostic> {
        let Some(recorded) = expr.expr_type.as_ref().and_then(|ty| ty.type_id()) else {
            return Ok(false);
        };
        let Some(ty) = self.lowered_type(recorded)? else {
            return Ok(false);
        };
        Ok(self
            .program
            .types()
            .value_range(ty)
            .is_some_and(|(min, max)| i128::from(i32::MIN) <= min && max <= i128::from(i32::MAX)))
    }
}
