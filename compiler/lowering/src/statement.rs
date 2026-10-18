//! Statements.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{
    Assignment, Expr as AstExpr, ExprKind as AstExprKind, StmtKind as AstStmtKind, Variable,
};
use ironplc_ir::lowered::{Expr, Place, Stmt, TypeKind};

use crate::variables::BodyFrame;
use crate::Lowering;

impl Lowering<'_> {
    /// Lowers the statement `stmt` of the body of `frame` to the statements
    /// it runs, in order, or `None` when lowering does not support it, or
    /// something inside it, yet. A function block call lowers to the stores
    /// of its inputs, the call and the stores of its outputs, a method call
    /// statement to the call and the stores of its outputs, and the
    /// assignment of a call of a function whose result is a structure to the
    /// call, the stores of its outputs and the copy of its result; every
    /// other statement lowers to one statement.
    pub(crate) fn statement(
        &mut self,
        frame: &BodyFrame,
        stmt: &AstStmtKind,
    ) -> Result<Option<Vec<Stmt>>, Diagnostic> {
        let one = |stmt: Result<Option<Stmt>, Diagnostic>| Ok(stmt?.map(|stmt| vec![stmt]));
        match stmt {
            AstStmtKind::Assignment(assignment) => {
                match self.structure_result_assignment(frame, assignment)? {
                    Some(statements) => Ok(Some(statements)),
                    None => one(self.assignment(frame, assignment)),
                }
            }
            AstStmtKind::If(stmt) => one(self.if_statement(frame, stmt)),
            AstStmtKind::Case(stmt) => one(self.case_statement(frame, stmt)),
            AstStmtKind::For(stmt) => one(self.for_statement(frame, stmt)),
            AstStmtKind::While(stmt) => one(self.while_statement(frame, stmt)),
            AstStmtKind::Repeat(stmt) => one(self.repeat_statement(frame, stmt)),
            AstStmtKind::Exit(span) => one(self.exit_statement(span)),
            AstStmtKind::Continue(span) => one(self.continue_statement(span)),
            // The AST keeps no span for a `RETURN`.
            AstStmtKind::Return => one(Ok(Some(Stmt::return_(SourceSpan::default())))),
            AstStmtKind::FbCall(call) => self.fb_call(frame, call),
            AstStmtKind::MethodCall(call) => self.method_call_statement(frame, call),
        }
    }

    /// `target := value`.
    pub(crate) fn assignment(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
    ) -> Result<Option<Stmt>, Diagnostic> {
        // `S=` and `R=` are not lowered yet. `r REF= x` is `r := REF(x)`:
        // the parser writes its value as `REF(x)`.
        if assignment.set_bind || assignment.reset_bind {
            return Ok(None);
        }
        self.store(
            frame,
            &assignment.target,
            assignment.deref,
            &assignment.value,
            assignment.span(),
        )
    }

    /// Stores the source value `value` at `target`, or, when `deref`, at what
    /// `target` refers to (`p^ := v`): selected bits of an integer, a string,
    /// or a scalar value. Where the target is stored narrower than its
    /// operation type, lowering narrows the value first, unless it already
    /// fits (REQ-LOW-lowering-046).
    pub(crate) fn store(
        &mut self,
        frame: &BodyFrame,
        target: &Variable,
        deref: bool,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Result<Option<Stmt>, Diagnostic> {
        if let Variable::Symbolic(target) = target {
            if crate::bits::selected_bits(target).is_some() {
                return self.bits_assignment(frame, target, deref, value, span);
            }
        }
        let Some(mut place) = self.variable_place(frame, target)? else {
            return Ok(None);
        };
        if deref {
            place = self.program.deref(place, &target.span())?;
        }
        self.store_at(frame, place, value, span)
    }

    /// Stores the source value `value` at `place`: a string, or a scalar
    /// value narrowed to the place's storage unless it already fits
    /// (REQ-LOW-lowering-046).
    pub(crate) fn store_at(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Result<Option<Stmt>, Diagnostic> {
        match self.program.types().kind(place.ty()) {
            Some(TypeKind::String(_)) => return self.string_assignment(frame, place, value, span),
            Some(
                TypeKind::Array { .. }
                | TypeKind::Structure { .. }
                | TypeKind::FunctionBlock { .. },
            ) => return self.aggregate_assignment(frame, place, value, span),
            Some(
                TypeKind::Bool
                | TypeKind::Integer { .. }
                | TypeKind::BitString { .. }
                | TypeKind::Real { .. }
                | TypeKind::Temporal { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Subrange { .. }
                | TypeKind::Program { .. }
                | TypeKind::Reference { .. },
            )
            | None => {}
        }
        let Some(operated) = self.program.types().operation_type(place.ty()) else {
            // A program instance, which is not assigned.
            return Ok(None);
        };
        let Some(value) = self.expr(frame, value)? else {
            return Ok(None);
        };
        if value.ty() != operated {
            // A reference, an interface value or a value the analyzer did
            // not convert to the target's type: the analyzer records every
            // conversion, so the last is a defect, which `Stmt::assign`
            // reports below.
            if !value.ty().is_numeric() || !operated.is_numeric() {
                return Ok(None);
            }
        }
        self.assign_value(place, value, span).map(Some)
    }

    /// Stores the lowered scalar `value` at `place`, narrowed to the place's
    /// storage unless it already fits (REQ-LOW-lowering-046).
    pub(crate) fn assign_value(
        &mut self,
        place: Place,
        mut value: Expr,
        span: SourceSpan,
    ) -> Result<Stmt, Diagnostic> {
        if let Some(width) = self.program.store_narrowing(place.ty(), &value) {
            let span = value.span().clone();
            value = Expr::truncate(width, value, span)?;
        }
        self.program.assign(place, value, span)
    }

    /// Copies the whole of the variable `value`, or of the output of a call
    /// `value` reads (`ob => f`), to `place`, an array, a structure or a
    /// function block instance (REQ-LOW-lowering-061). Any other source is
    /// not lowered here; the assignment of a call of a function that returns
    /// a structure is `structure_result.rs`'s.
    fn aggregate_assignment(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Result<Option<Stmt>, Diagnostic> {
        let source = if let AstExprKind::CallOutput(output) = &value.kind {
            self.call_output_place(output)?
        } else if let AstExprKind::Variable(variable) = &value.kind {
            self.variable_place(frame, variable)?
        } else {
            return Ok(None);
        };
        let Some(source) = source else {
            return Ok(None);
        };
        self.program.copy(place, source, span).map(Some)
    }
}
