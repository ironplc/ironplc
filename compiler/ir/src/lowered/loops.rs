//! Loops: `Loop`, `For`, and the `Exit` and `Continue` that name them.
//!
//! A loop is opened before its body is built and closed by building the loop
//! from it. Only the open loop's [`LoopScope`] makes an `Exit` or `Continue`
//! naming the loop, and closing the loop checks that every one it made is
//! inside the loop's body. So every `Exit` and `Continue` names a loop that
//! encloses it (REQ-LOW-lowering-062).

use std::cell::Cell;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::expr::{BinaryOp, CompareOp, Const, Expr};
use super::ids::LoopId;
use super::invariant::broken;
use super::place::{Place, Projection};
use super::scalar::ScalarType;
use super::stmt::{one_program, programs, store_narrowing, Stmt, StmtKind};
use super::types::TypeTable;

/// A loop whose body is being built.
#[derive(Debug)]
pub struct LoopScope {
    id: LoopId,
    /// How many `Exit` and `Continue` statements naming the loop the scope
    /// has made.
    made: Cell<u32>,
}

impl LoopScope {
    pub(crate) fn new(id: LoopId) -> Self {
        Self {
            id,
            made: Cell::new(0),
        }
    }

    /// The loop the scope builds.
    pub fn id(&self) -> LoopId {
        self.id
    }

    /// Leaves the loop.
    pub fn exit(&self, span: SourceSpan) -> Stmt {
        self.made.set(self.made.get() + 1);
        Stmt::checked(StmtKind::Exit(self.id), span, None)
    }

    /// Goes on with the loop's next iteration.
    pub fn continue_(&self, span: SourceSpan) -> Stmt {
        self.made.set(self.made.get() + 1);
        Stmt::checked(StmtKind::Continue(self.id), span, None)
    }

    /// Fails unless every `Exit` and `Continue` the scope made is in `stmts`.
    fn check_enclosed<'a>(
        &self,
        constructor: &str,
        span: &SourceSpan,
        stmts: impl Iterator<Item = &'a Stmt>,
    ) -> Result<(), Diagnostic> {
        let found: u32 = stmts.map(|stmt| naming(self.id, stmt)).sum();
        if found != self.made.get() {
            return Err(broken(
                constructor,
                span,
                format!(
                    "{} of the exits and continues naming {} are outside it",
                    self.made.get().saturating_sub(found),
                    self.id
                ),
            ));
        }
        Ok(())
    }
}

/// How many `Exit` and `Continue` statements naming `id` are in `stmt`.
fn naming(id: LoopId, stmt: &Stmt) -> u32 {
    let all = |stmts: &[Stmt]| stmts.iter().map(|stmt| naming(id, stmt)).sum::<u32>();
    match stmt.kind() {
        StmtKind::Exit(named) | StmtKind::Continue(named) => u32::from(*named == id),
        StmtKind::If {
            then, otherwise, ..
        } => all(then) + all(otherwise),
        StmtKind::Case {
            arms, otherwise, ..
        } => arms.iter().map(|arm| all(arm.body())).sum::<u32>() + all(otherwise),
        StmtKind::For { body, .. } => all(body),
        StmtKind::Loop {
            body, continuing, ..
        } => all(body) + all(continuing),
        StmtKind::Assign { .. }
        | StmtKind::AssignBits { .. }
        | StmtKind::AssignStr { .. }
        | StmtKind::Copy { .. }
        | StmtKind::FbCall { .. }
        | StmtKind::Call { .. }
        | StmtKind::Return => 0,
    }
}

/// Repeats `body` then `continuing` until an `Exit` leaves the loop.
pub(crate) fn build_loop(
    scope: LoopScope,
    body: Vec<Stmt>,
    continuing: Vec<Stmt>,
    span: SourceSpan,
) -> Result<Stmt, Diagnostic> {
    scope.check_enclosed("Stmt::loop_", &span, body.iter().chain(&continuing))?;
    let program = one_program(
        "Stmt::loop_",
        &span,
        programs(&body).chain(programs(&continuing)),
    )?;
    Ok(Stmt::checked(
        StmtKind::Loop {
            id: scope.id,
            body,
            continuing,
        },
        span,
        program,
    ))
}

/// The parts of a `For` other than its id.
pub struct ForParts {
    /// The variable, field of the current instance, or what a reference
    /// variable refers to, that counts.
    pub control: Place,
    /// The first value of `control`, already narrowed to its storage.
    pub from: Expr,
    /// The bound, evaluated on every iteration.
    pub to: Expr,
    /// What each iteration adds to `control`.
    pub step: Const,
    pub body: Vec<Stmt>,
}

/// Counts `control` from `from` to `to` by `step`, running `body` each time.
///
/// Whether `place` can be a `For`'s control variable: a variable, a field of
/// the current instance, or what a reference variable refers to, such as a
/// function's `VAR_OUTPUT` parameter (REQ-LOW-lowering-067).
pub fn is_loop_control(place: &Place) -> bool {
    matches!(
        place.path(),
        [] | [Projection::Deref] | [Projection::Deref, Projection::Field(_)]
    )
}

/// `control` is a variable, a field of the current instance or what a
/// reference variable refers to, whose operation type is an integer; `from`
/// and `to` have that type, `from` already fits `control`'s storage, and
/// `step` is a nonzero constant of it (REQ-LOW-lowering-066,
/// REQ-LOW-lowering-067).
pub(crate) fn build_for(
    types: &TypeTable,
    scope: LoopScope,
    parts: ForParts,
    span: SourceSpan,
) -> Result<Stmt, Diagnostic> {
    const CONSTRUCTOR: &str = "Stmt::for_";
    let ForParts {
        control,
        from,
        to,
        step,
        body,
    } = parts;
    let refuse = |message: String| Err(broken(CONSTRUCTOR, &span, message));
    let Some(ty) = types
        .operation_type(control.ty())
        .filter(|ty| ty.is_integer())
    else {
        return refuse(format!("the control variable {control} is not an integer"));
    };
    if !is_loop_control(&control) {
        return refuse(format!(
            "the control variable {control} is not a variable, a field of the instance or what a reference refers to"
        ));
    }
    if from.ty() != ty || to.ty() != ty {
        return refuse(format!(
            "the bounds are a {} and a {}, not a {ty}",
            from.ty(),
            to.ty()
        ));
    }
    if store_narrowing(types, control.ty(), &from).is_some() {
        return refuse(format!("the first value does not fit {control}"));
    }
    if !step.has_width_of(ty) || step.integer_value(ty) == Some(0) {
        return refuse(format!("the step {step:?} is not a nonzero {ty}"));
    }
    scope.check_enclosed(CONSTRUCTOR, &span, body.iter())?;
    let program = one_program(
        CONSTRUCTOR,
        &span,
        [Some(control.stamp()), from.program(), to.program()]
            .into_iter()
            .chain(programs(&body)),
    )?;
    Ok(Stmt::checked(
        StmtKind::For {
            id: scope.id,
            control,
            from,
            to,
            step,
            body,
        },
        span,
        program,
    ))
}

/// Whether a `For` with `step`, over a control of operation type `ty`,
/// counts down.
pub fn counts_down(step: Const, ty: ScalarType) -> bool {
    step.integer_value(ty).is_some_and(|value| value < 0)
}

/// `stmt` with a `For` expanded into an `Assign` and a `Loop` of the same
/// meaning (REQ-LOW-ir-068); any other statement is returned as it is.
///
/// The `Loop` keeps the `For`'s id, so an `Exit` or `Continue` in the body
/// names it. Its body first leaves when the control is past the bound, then
/// runs the `For`'s body; its `continuing` adds the step, wrapped to the
/// control's storage.
pub(crate) fn expand_for(types: &TypeTable, stmt: Stmt) -> Result<Vec<Stmt>, Diagnostic> {
    let span = stmt.span().clone();
    let program = stmt.program();
    let StmtKind::For {
        id,
        control,
        from,
        to,
        step,
        body,
    } = stmt.kind().clone()
    else {
        return Ok(vec![stmt]);
    };
    let ty = to.ty();
    let read = |span: &SourceSpan| Expr::read(types, control.clone(), span.clone());
    let start = Stmt::assign(types, control.clone(), from, span.clone())?;
    let within = Expr::compare(
        if counts_down(step, ty) {
            CompareOp::Ge
        } else {
            CompareOp::Le
        },
        read(&span)?,
        to,
        span.clone(),
    )?;
    let head = Stmt::if_(
        within,
        vec![],
        vec![Stmt::checked(StmtKind::Exit(id), span.clone(), None)],
        span.clone(),
    )?;
    let mut next = Expr::binary(
        BinaryOp::Add,
        read(&span)?,
        Expr::constant(step, ty, span.clone())?,
        span.clone(),
    )?;
    if let Some(width) = store_narrowing(types, control.ty(), &next) {
        next = Expr::truncate(width, next, span.clone())?;
    }
    let advance = Stmt::assign(types, control, next, span.clone())?;
    let mut loop_body = vec![head];
    loop_body.extend(body);
    let repeat = Stmt::checked(
        StmtKind::Loop {
            id,
            body: loop_body,
            continuing: vec![advance],
        },
        span,
        program,
    );
    Ok(vec![start, repeat])
}
