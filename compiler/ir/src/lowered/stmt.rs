//! Statements.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::expr::{Const, Expr, ExprKind};
use super::ids::{LoopId, Stamp, TypeId};
use super::invariant::broken;
use super::place::Place;
use super::scalar::ScalarType;
use super::types::{NarrowWidth, Nullability, TypeTable};

/// What a statement does.
#[derive(Clone, Debug)]
pub enum StmtKind {
    /// Stores a scalar value at a place.
    Assign { place: Place, value: Expr },
    /// Runs `then` when `cond` is true and `otherwise` when it is false.
    If {
        cond: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
    },
    /// Evaluates `selector` once and runs the first arm with a label that
    /// matches it, or `otherwise` when none does.
    Case {
        selector: Expr,
        arms: Vec<CaseArm>,
        otherwise: Vec<Stmt>,
    },
    /// Counts `control` from `from` to `to` by `step` (REQ-LOW-lowering-066).
    For {
        id: LoopId,
        control: Place,
        from: Expr,
        to: Expr,
        step: Const,
        body: Vec<Stmt>,
    },
    /// Repeats `body` then `continuing` until an `Exit` leaves it. A
    /// `Continue` skips the rest of `body` and goes on to `continuing`.
    Loop {
        id: LoopId,
        body: Vec<Stmt>,
        continuing: Vec<Stmt>,
    },
    /// Leaves the enclosing `Loop` or `For` it names.
    Exit(LoopId),
    /// Goes on with the next iteration of the enclosing `Loop` or `For` it
    /// names.
    Continue(LoopId),
    /// Returns from the body.
    Return,
}

/// One arm of a `Case`: its labels, and the statements it runs.
#[derive(Clone, Debug)]
pub struct CaseArm {
    labels: Vec<CaseLabel>,
    body: Vec<Stmt>,
}

impl CaseArm {
    /// The arm that runs `body` when the selector matches one of `labels`.
    /// The `Case` it belongs to checks the labels against its selector.
    pub fn new(labels: Vec<CaseLabel>, body: Vec<Stmt>) -> Self {
        Self { labels, body }
    }

    pub fn labels(&self) -> &[CaseLabel] {
        &self.labels
    }

    pub fn body(&self) -> &[Stmt] {
        &self.body
    }
}

/// A label of a `Case` arm (REQ-LOW-lowering-064).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaseLabel {
    /// Matches the selector equal to the value.
    Value(Const),
    /// Matches the selector from the first value to the second, both included.
    Range(Const, Const),
}

/// A statement and the span of the source statement it stands for
/// (REQ-LOW-lowering-063).
#[derive(Clone, Debug)]
pub struct Stmt {
    kind: StmtKind,
    span: SourceSpan,
    /// The program whose places the statement names; `None` when it names
    /// none.
    program: Option<Stamp>,
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
    pub(crate) fn assign(
        types: &TypeTable,
        place: Place,
        value: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        if value
            .program()
            .is_some_and(|program| program != place.stamp())
        {
            return Err(broken(
                "Stmt::assign",
                &span,
                "the value reads places of another program",
            ));
        }
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
        if let Some(width) = store_narrowing(types, place.ty(), &value) {
            return Err(broken(
                "Stmt::assign",
                &span,
                format!(
                    "the value is not narrowed to the {} bits of {}",
                    width.bits(),
                    place.ty()
                ),
            ));
        }
        let program = Some(place.stamp());
        Ok(Self {
            kind: StmtKind::Assign { place, value },
            span,
            program,
        })
    }

    /// Runs `then` when `cond`, a `BOOL`, is true, and `otherwise` when it is
    /// false.
    pub fn if_(
        cond: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        if cond.ty() != ScalarType::Bool {
            return Err(broken(
                "Stmt::if_",
                &span,
                format!("the condition is a {}, not a bool", cond.ty()),
            ));
        }
        let program = one_program(
            "Stmt::if_",
            &span,
            std::iter::once(cond.program()).chain(programs(&then).chain(programs(&otherwise))),
        )?;
        Ok(Self {
            kind: StmtKind::If {
                cond,
                then,
                otherwise,
            },
            span,
            program,
        })
    }

    /// Runs the first of `arms` with a label matching `selector`, an integer,
    /// or `otherwise` when none does. Every label is a constant of the
    /// selector's type, and a range runs from its lower value to its higher
    /// one (REQ-LOW-lowering-064).
    pub fn case(
        selector: Expr,
        arms: Vec<CaseArm>,
        otherwise: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let ty = selector.ty();
        if !ty.is_integer() {
            return Err(broken(
                "Stmt::case",
                &span,
                format!("the selector is a {ty}, not an integer"),
            ));
        }
        let of_selector = |constant: &Const| constant.has_width_of(ty);
        for label in arms.iter().flat_map(|arm| &arm.labels) {
            let fits = match label {
                CaseLabel::Value(value) => of_selector(value),
                CaseLabel::Range(low, high) => {
                    of_selector(low)
                        && of_selector(high)
                        && low.integer_value(ty) <= high.integer_value(ty)
                }
            };
            if !fits {
                return Err(broken(
                    "Stmt::case",
                    &span,
                    format!("the label {label:?} is not a constant or range of {ty}"),
                ));
            }
        }
        let program = one_program(
            "Stmt::case",
            &span,
            std::iter::once(selector.program()).chain(
                arms.iter()
                    .flat_map(|arm| programs(&arm.body))
                    .chain(programs(&otherwise)),
            ),
        )?;
        Ok(Self {
            kind: StmtKind::Case {
                selector,
                arms,
                otherwise,
            },
            span,
            program,
        })
    }

    /// Returns from the body.
    pub fn return_(span: SourceSpan) -> Self {
        Self {
            kind: StmtKind::Return,
            span,
            program: None,
        }
    }

    /// A statement whose invariants its builder has already checked.
    pub(crate) fn checked(kind: StmtKind, span: SourceSpan, program: Option<Stamp>) -> Self {
        Self {
            kind,
            span,
            program,
        }
    }

    /// The program whose places the statement names.
    pub(crate) fn program(&self) -> Option<Stamp> {
        self.program
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
pub fn store_narrowing(types: &TypeTable, place: TypeId, value: &Expr) -> Option<NarrowWidth> {
    let width = types.narrowed_storage(place)?;
    (!fits(types, place, width, value)).then_some(width)
}

/// Whether `value` already fits a place of type `place`, stored at `width`.
fn fits(types: &TypeTable, place: TypeId, width: NarrowWidth, value: &Expr) -> bool {
    let Some((min, max)) = types.value_range(place) else {
        return false;
    };
    match value.kind() {
        ExprKind::Truncate { width: to, .. } => *to == width,
        ExprKind::Const(constant) => constant
            .integer_value(value.ty())
            .is_some_and(|int| (min..=max).contains(&int)),
        ExprKind::Read(source) => types
            .value_range(source.ty())
            .is_some_and(|(low, high)| min <= low && high <= max),
        ExprKind::Convert(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. } => false,
    }
}

/// The programs whose places `stmts` name.
pub(crate) fn programs(stmts: &[Stmt]) -> impl Iterator<Item = Option<Stamp>> + '_ {
    stmts.iter().map(Stmt::program)
}

/// The one program `parts` name places of, or a P9998 naming `constructor`
/// when they name places of two.
pub(crate) fn one_program(
    constructor: &str,
    span: &SourceSpan,
    parts: impl Iterator<Item = Option<Stamp>>,
) -> Result<Option<Stamp>, Diagnostic> {
    let mut found = None;
    for program in parts.flatten() {
        match found {
            Some(other) if other != program => {
                return Err(broken(
                    constructor,
                    span,
                    "the statement names places of two programs",
                ))
            }
            Some(_) | None => found = Some(program),
        }
    }
    Ok(found)
}
