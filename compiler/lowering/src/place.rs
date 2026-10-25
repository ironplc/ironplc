//! Places named in the source: a variable, a field of a structure, an
//! element of an array, and what a reference refers to.

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::textual::{Expr as AstExpr, SymbolicVariableKind};
use ironplc_ir::lowered::{Expr, Place};

use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// The place `symbolic` names in the body of `frame`. The bits a bit or
    /// partial access selects are read from and written to a place rather
    /// than being one (`bits.rs`), and `THIS^` and `SUPER^` are not lowered
    /// yet.
    pub(crate) fn symbolic_place(
        &mut self,
        frame: &BodyFrame,
        symbolic: &SymbolicVariableKind,
    ) -> Lowered<Place> {
        match symbolic {
            SymbolicVariableKind::Named(named) => {
                self.named_place(frame, &named.name, &named.name.span())
            }
            SymbolicVariableKind::Structured(structured) => {
                let record = self.symbolic_place(frame, &structured.record)?;
                let position = self
                    .program
                    .types()
                    .fields(record.ty())
                    .and_then(|fields| {
                        fields
                            .iter()
                            .position(|field| Id::from(&field.name.to_string()) == structured.field)
                    })
                    .ok_or(Unlowered::NotYet)?;
                let field = self
                    .program
                    .field_idx(record.ty(), position)
                    .ok_or(Unlowered::NotYet)?;
                Ok(self
                    .program
                    .field(record, field, &structured.field.span())?)
            }
            SymbolicVariableKind::Array(array) => {
                let mut base = self.symbolic_place(frame, &array.subscripted_variable)?;
                // A subscript of a reference to an array indexes the array it
                // refers to, as `r^[i]` does: a TwinCAT `REFERENCE TO`, and a
                // `REF_TO` the analyzer lets a subscript follow without `^`.
                if self.program.types().reference(base.ty()).is_some() {
                    base = self.program.deref(base, &array.span())?;
                }
                let mut subscripts = Vec::with_capacity(array.subscripts.len());
                for subscript in &array.subscripts {
                    subscripts.push(self.subscript(frame, subscript)?);
                }
                Ok(self.program.index(base, subscripts, &array.span())?)
            }
            SymbolicVariableKind::Deref(deref) => {
                let reference = self.symbolic_place(frame, &deref.variable)?;
                Ok(self.program.deref(reference, &deref.span())?)
            }
            SymbolicVariableKind::BitAccess(_)
            | SymbolicVariableKind::PartialAccess(_)
            | SymbolicVariableKind::SelfRef(_) => Err(Unlowered::NotYet),
        }
    }

    /// A subscript, a `DINT` (REQ-LOW-lowering-161). The analyzer converts a
    /// subscript of another type to `DINT`, by a checked conversion for a
    /// type that holds values a `DINT` does not (REQ-LOW-analyzer-162), and
    /// rejects a `BOOL` one (P4086). A subscript of another type the analyzer
    /// did not convert is a defect, which building the `Index` reports
    /// (REQ-LOW-lowering-094).
    fn subscript(&mut self, frame: &BodyFrame, subscript: &AstExpr) -> Lowered<Expr> {
        self.expr(frame, subscript)
    }
}
