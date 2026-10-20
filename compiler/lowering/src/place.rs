//! Places named in the source: a variable, a field of a structure, an
//! element of an array, and what a reference refers to.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, SymbolicVariableKind};
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
                let Some(mut base) = self.symbolic_place(frame, &array.subscripted_variable)?
                else {
                    return Ok(None);
                };
                // A subscript of a reference to an array indexes the array it
                // refers to, as `r^[i]` does: a TwinCAT `REFERENCE TO`, and a
                // `REF_TO` the analyzer lets a subscript follow without `^`.
                if self.program.types().reference(base.ty()).is_some() {
                    base = self.program.deref(base, &array.span())?;
                }
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
    /// subscript of another type to `DINT`, by a checked conversion for a
    /// type that holds values a `DINT` does not (REQ-LOW-analyzer-162), and
    /// rejects a `BOOL` one (P4086). A subscript of another type the analyzer
    /// did not convert is a defect, which building the `Index` reports
    /// (REQ-LOW-lowering-094).
    fn subscript(&mut self, frame: &BodyFrame, subscript: &AstExpr) -> Made<Expr> {
        self.expr(frame, subscript)
    }
}
