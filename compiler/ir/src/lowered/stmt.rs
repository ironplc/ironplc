//! Statements.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::{program_of, Arg, Callee};
use super::expr::{check_user_call, Const, Expr, ExprKind};
use super::ids::{LoopId, PouId, Stamp, TypeId};
use super::invariant::broken;
use super::place::Place;
use super::scalar::ScalarType;
use super::str_expr::StrExpr;
use super::types::{NarrowWidth, Nullability, TypeKind, TypeTable};
use super::variables::{PouTable, VariableTable};

/// What a statement does.
#[derive(Clone, Debug)]
pub enum StmtKind {
    /// Stores a scalar value at a place.
    Assign { place: Place, value: Expr },
    /// Stores a string at a place of a string type.
    AssignStr { place: Place, value: StrExpr },
    /// Replaces the `bits` bits of the value at `place` that start at bit
    /// `shift` with the low `bits` bits of `value`, leaving the others as
    /// they were. The place is named once, so it is evaluated once.
    AssignBits {
        place: Place,
        shift: u8,
        bits: u8,
        value: Expr,
    },
    /// Replaces the whole value at `dst`, an array, a structure or a function
    /// block instance, with the whole value at `src` (REQ-LOW-lowering-061).
    Copy { dst: Place, src: Place },
    /// Runs the body of the function block `block` on `instance`, an
    /// instance of it. A call's inputs and outputs are the assignments
    /// around it (REQ-LOW-lowering-069).
    FbCall { instance: Place, block: PouId },
    /// Calls `callee` with `args`, discarding its result, if any. A call of
    /// a method is one, wherever its result is not used.
    Call { callee: Callee, args: Vec<Arg> },
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

    /// Stores the string `value` at `place`, a string of the same encoding
    /// (REQ-LOW-lowering-050). A value longer than the place's capacity keeps
    /// its first code units (REQ-LOW-codegen-056).
    pub(crate) fn assign_str(
        types: &TypeTable,
        place: Place,
        value: StrExpr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "Stmt::assign_str";
        if value
            .program()
            .is_some_and(|program| program != place.stamp())
        {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                "the value reads places of another program",
            ));
        }
        let Some(TypeKind::String(shape)) = types.kind(place.ty()) else {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("{} is not a string", place.ty()),
            ));
        };
        if shape.encoding != value.shape().encoding {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!(
                    "a {:?} string cannot be stored at {}",
                    value.shape().encoding,
                    place.ty()
                ),
            ));
        }
        let program = Some(place.stamp());
        Ok(Self {
            kind: StmtKind::AssignStr { place, value },
            span,
            program,
        })
    }

    /// Replaces the `bits` bits of the value at `place` that start at bit
    /// `shift` with the low `bits` bits of `value`.
    ///
    /// The place's type is an integer or a bit string, and the bits lie
    /// within its width (REQ-LOW-lowering-065). The value is a `BOOL` for one
    /// bit, a 32-bit integer for up to 32 bits, and a 64-bit integer for
    /// more.
    pub(crate) fn assign_bits(
        types: &TypeTable,
        place: Place,
        shift: u8,
        bits: u8,
        value: Expr,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "Stmt::assign_bits";
        let refuse = |message: String| Err(broken(CONSTRUCTOR, &span, message));
        if value
            .program()
            .is_some_and(|program| program != place.stamp())
        {
            return refuse("the value reads places of another program".to_string());
        }
        let Some(width) = types.bit_width(place.ty()) else {
            return refuse(format!("{} holds no bits to assign", place.ty()));
        };
        if bits == 0 || u16::from(shift) + u16::from(bits) > u16::from(width) {
            return refuse(format!(
                "{bits} bits from bit {shift} do not lie within the {width} bits of {}",
                place.ty()
            ));
        }
        let holds = match value.ty() {
            ScalarType::Bool => bits == 1,
            ScalarType::I32(_) => bits <= 32,
            ScalarType::I64(_) => bits > 32,
            ScalarType::F32 | ScalarType::F64 | ScalarType::Ref | ScalarType::Interface => false,
        };
        if !holds {
            return refuse(format!(
                "a {} value is not assigned to {bits} bits",
                value.ty()
            ));
        }
        let program = Some(place.stamp());
        Ok(Self {
            kind: StmtKind::AssignBits {
                place,
                shift,
                bits,
                value,
            },
            span,
            program,
        })
    }

    /// Replaces the whole value at `dst` with the whole value at `src`.
    ///
    /// `dst` is an array, a structure or a function block instance, and `src`
    /// has its type or, for an array or a structure, a type of the same shape
    /// (REQ-LOW-lowering-061, [`TypeTable::same_shape`]). Both name places of
    /// one program.
    pub(crate) fn copy(
        types: &TypeTable,
        dst: Place,
        src: Place,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "Stmt::copy";
        if src.stamp() != dst.stamp() {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                "the source is a place of another program",
            ));
        }
        let aggregate = match types.kind(dst.ty()) {
            Some(TypeKind::Array { .. } | TypeKind::Structure { .. }) => {
                types.same_shape(dst.ty(), src.ty())
            }
            Some(TypeKind::FunctionBlock { .. }) => dst.ty() == src.ty(),
            Some(
                TypeKind::Bool
                | TypeKind::Integer { .. }
                | TypeKind::BitString { .. }
                | TypeKind::Real { .. }
                | TypeKind::Temporal { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Subrange { .. }
                | TypeKind::String(_)
                | TypeKind::Program { .. }
                | TypeKind::Reference { .. },
            )
            | None => {
                return Err(broken(
                    CONSTRUCTOR,
                    &span,
                    format!("{} is not an array, a structure or a block", dst.ty()),
                ))
            }
        };
        if !aggregate {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("a value of {} cannot be copied to {}", src.ty(), dst.ty()),
            ));
        }
        let program = Some(dst.stamp());
        Ok(Self {
            kind: StmtKind::Copy { dst, src },
            span,
            program,
        })
    }

    /// Runs the body of the function block `instance` is an instance of, on
    /// `instance`. The block is the body of the place's type, so the two
    /// cannot disagree (REQ-LOW-ir-171).
    pub(crate) fn fb_call(
        types: &TypeTable,
        instance: Place,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let Some(TypeKind::FunctionBlock { body, .. }) = types.kind(instance.ty()) else {
            return Err(broken(
                "Stmt::fb_call",
                &span,
                format!("{} is not a function block", instance.ty()),
            ));
        };
        let block = *body;
        let program = Some(instance.stamp());
        Ok(Self {
            kind: StmtKind::FbCall { instance, block },
            span,
            program,
        })
    }

    /// Calls the function or method `pou` that `pous` declares with `args`,
    /// which [`check_user_call`] describes, discarding its result, if any.
    pub(crate) fn call(
        types: &TypeTable,
        variables: &VariableTable,
        pous: &PouTable,
        pou: PouId,
        args: Vec<Arg>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::call_stmt";
        check_user_call(CONSTRUCTOR, types, variables, pous, pou, &args, &span)?;
        let program = program_of(CONSTRUCTOR, &args, &span)?;
        Ok(Self {
            kind: StmtKind::Call {
                callee: Callee::User(pou),
                args,
            },
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
        | ExprKind::ConvertChecked(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => false,
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
