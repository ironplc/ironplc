//! Places named in the source: a variable, a field of a structure, an
//! element of an array, and what a reference refers to.

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::textual::{Expr as AstExpr, SymbolicVariableKind};
use ironplc_ir::lowered::{Expr, Field, FieldIdx, Place};

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
                self.field_of(record, &structured.field, &structured.field.span())
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

    /// The field `name` of the record at `record`, named at `span`. A record
    /// whose type has no field of that name is not lowered.
    pub(crate) fn field_of(&self, record: Place, name: &Id, span: &SourceSpan) -> Lowered<Place> {
        let (field, _) = self.field_named(&record, name).ok_or(Unlowered::NotYet)?;
        Ok(self.program.field(record, field, span)?)
    }

    /// The field `name` of the record at `record`, matched without regard to
    /// case as IEC identifiers are, with its index; `None` when the type of
    /// `record` is no record, or has no field of that name.
    pub(crate) fn field_named(&self, record: &Place, name: &Id) -> Option<(FieldIdx, &Field)> {
        let fields = self.program.types().fields(record.ty())?;
        let position = fields
            .iter()
            .position(|field| Id::from(&field.name.to_string()) == *name)?;
        let field = self.program.field_idx(record.ty(), position)?;
        Some((field, &fields[position]))
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
