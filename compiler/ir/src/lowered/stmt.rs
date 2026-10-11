//! Statements.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::expr::{Const, Expr, ExprKind};
use super::ids::TypeId;
use super::invariant::broken;
use super::place::Place;
use super::types::{Nullability, TypeTable};

/// What a statement does.
#[derive(Clone, Debug)]
pub enum StmtKind {
    /// Stores a scalar value at a place.
    Assign { place: Place, value: Expr },
}

/// A statement and the span of the source statement it stands for
/// (REQ-LOW-lowering-063).
#[derive(Clone, Debug)]
pub struct Stmt {
    kind: StmtKind,
    span: SourceSpan,
}

impl Stmt {
    /// Stores `value` at `place`.
    ///
    /// The value has the operation type of the place's type
    /// (REQ-LOW-lowering-060). Where the place is stored narrower than that
    /// type, the value already fits it: it is a `Truncate` to the place's
    /// width, a constant within the place's range, or a read of a place whose
    /// range lies within it (REQ-LOW-lowering-046). The place is not a
    /// non-null reference, which only a call binds (REQ-LOW-lowering-036).
    pub fn assign(
        types: &TypeTable,
        place: Place,
        value: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let operated = types.operation_type(place.ty());
        if operated != Some(value.ty()) {
            return Err(broken(
                "Stmt::assign",
                &span,
                format!("a {} value cannot be stored at {}", value.ty(), place.ty()),
            ));
        }
        if matches!(types.reference(place.ty()), Some((_, Nullability::NonNull))) {
            return Err(broken(
                "Stmt::assign",
                &span,
                "a non-null reference is bound only by a call",
            ));
        }
        if let Some(bits) = store_narrowing(types, place.ty(), &value) {
            return Err(broken(
                "Stmt::assign",
                &span,
                format!(
                    "the value is not narrowed to the {bits} bits of {}",
                    place.ty()
                ),
            ));
        }
        Ok(Self {
            kind: StmtKind::Assign { place, value },
            span,
        })
    }

    pub fn kind(&self) -> &StmtKind {
        &self.kind
    }

    pub fn span(&self) -> &SourceSpan {
        &self.span
    }
}

/// The width a `Truncate` must narrow `value` to before it is stored at a
/// place of type `place`; `None` when the value needs no narrowing, because
/// the place is not stored narrower than its operation type or the value
/// already fits it (REQ-LOW-lowering-046).
pub fn store_narrowing(types: &TypeTable, place: TypeId, value: &Expr) -> Option<u8> {
    let bits = types.narrowed_storage_bits(place)?;
    (!fits(types, place, bits, value)).then_some(bits)
}

/// Whether `value` already fits a place of type `place`, stored in `bits`
/// bits.
fn fits(types: &TypeTable, place: TypeId, bits: u8, value: &Expr) -> bool {
    let Some((min, max)) = types.value_range(place) else {
        return false;
    };
    match value.kind() {
        ExprKind::Truncate { bits: to, .. } => *to == bits,
        ExprKind::Const(Const::Int(int)) => (min..=max).contains(int),
        ExprKind::Read(source) => types
            .value_range(source.ty())
            .is_some_and(|(low, high)| min <= low && high <= max),
        ExprKind::Const(Const::Real(_))
        | ExprKind::Convert(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. } => false,
    }
}
