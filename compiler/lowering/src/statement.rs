//! Statements.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Assignment, StmtKind as AstStmtKind};
use ironplc_ir::lowered::{Expr, Stmt};

use crate::variables::BodyFrame;
use crate::Lowering;

impl Lowering<'_> {
    /// Lowers the statement `stmt` of the body of `frame`, or `None` when
    /// lowering does not support it, or something inside it, yet.
    pub(crate) fn statement(
        &mut self,
        frame: &BodyFrame,
        stmt: &AstStmtKind,
    ) -> Result<Option<Stmt>, Diagnostic> {
        match stmt {
            AstStmtKind::Assignment(assignment) => self.assignment(frame, assignment),
            AstStmtKind::If(stmt) => self.if_statement(frame, stmt),
            AstStmtKind::Case(stmt) => self.case_statement(frame, stmt),
            AstStmtKind::For(stmt) => self.for_statement(frame, stmt),
            AstStmtKind::While(stmt) => self.while_statement(frame, stmt),
            AstStmtKind::Repeat(stmt) => self.repeat_statement(frame, stmt),
            AstStmtKind::Exit(span) => self.exit_statement(span),
            AstStmtKind::Continue(span) => self.continue_statement(span),
            // The AST keeps no span for a `RETURN`.
            AstStmtKind::Return => Ok(Some(Stmt::return_(SourceSpan::default()))),
            AstStmtKind::FbCall(_) | AstStmtKind::MethodCall(_) => Ok(None),
        }
    }

    /// `target := value`, storing a scalar value. Where the target is stored
    /// narrower than its operation type, lowering narrows the value first,
    /// unless it already fits (REQ-LOW-lowering-046).
    fn assignment(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
    ) -> Result<Option<Stmt>, Diagnostic> {
        // `REF=`, `S=` and `R=` are not lowered yet.
        if assignment.ref_bind || assignment.set_bind || assignment.reset_bind {
            return Ok(None);
        }
        let Some(mut place) = self.variable_place(frame, &assignment.target)? else {
            return Ok(None);
        };
        // `p^ := v` stores in what `p` refers to.
        if assignment.deref {
            place = self.program.deref(place, &assignment.target.span())?;
        }
        let Some(operated) = self.program.types().operation_type(place.ty()) else {
            // A string or an aggregate is not assigned as one scalar value.
            return Ok(None);
        };
        let Some(mut value) = self.expr(frame, &assignment.value)? else {
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
        if let Some(width) = self.program.store_narrowing(place.ty(), &value) {
            let span = value.span().clone();
            value = Expr::truncate(width, value, span)?;
        }
        self.program
            .assign(place, value, assignment.span())
            .map(Some)
    }
}
