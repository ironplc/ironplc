//! Statements.

use ironplc_dsl::core::Located;
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
            AstStmtKind::FbCall(_)
            | AstStmtKind::MethodCall(_)
            | AstStmtKind::If(_)
            | AstStmtKind::Case(_)
            | AstStmtKind::For(_)
            | AstStmtKind::While(_)
            | AstStmtKind::Repeat(_)
            | AstStmtKind::Return
            | AstStmtKind::Exit(_)
            | AstStmtKind::Continue(_) => Ok(None),
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
        // `REF=`, `S=`, `R=` and an assignment through `^` are not lowered yet.
        if assignment.deref || assignment.ref_bind || assignment.set_bind || assignment.reset_bind {
            return Ok(None);
        }
        let Some(place) = self.variable_place(frame, &assignment.target)? else {
            return Ok(None);
        };
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
