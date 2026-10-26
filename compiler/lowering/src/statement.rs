//! Statements.

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{
    Assignment, Expr as AstExpr, ExprKind as AstExprKind, StmtKind as AstStmtKind, Variable,
};
use ironplc_ir::lowered::{Expr, Place, Stmt, TypeKind};

use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// Lowers the statement `stmt` of the body of `frame` to the statements
    /// it runs, in order. A function block call lowers to the stores of its
    /// inputs, the call and the stores of its outputs, a method call
    /// statement to the call and the stores of its outputs, and the
    /// assignment of a call of a function whose result is a structure to the
    /// call, the stores of its outputs and the copy of its result; every
    /// other statement lowers to one statement.
    pub(crate) fn statement(
        &mut self,
        frame: &BodyFrame,
        stmt: &AstStmtKind,
    ) -> Lowered<Vec<Stmt>> {
        let one = |stmt: Lowered<Stmt>| stmt.map(|stmt| vec![stmt]);
        match stmt {
            AstStmtKind::Assignment(assignment) => self.assignment(frame, assignment),
            AstStmtKind::If(stmt) => one(self.if_statement(frame, stmt)),
            AstStmtKind::Case(stmt) => one(self.case_statement(frame, stmt)),
            AstStmtKind::For(stmt) => one(self.for_statement(frame, stmt)),
            AstStmtKind::While(stmt) => one(self.while_statement(frame, stmt)),
            AstStmtKind::Repeat(stmt) => one(self.repeat_statement(frame, stmt)),
            AstStmtKind::Exit(span) => one(self.exit_statement(frame, span)),
            AstStmtKind::Continue(span) => one(self.continue_statement(frame, span)),
            // The AST keeps no span for a `RETURN`.
            AstStmtKind::Return => Ok(vec![Stmt::return_(SourceSpan::default())]),
            AstStmtKind::FbCall(call) => self.fb_call(frame, call),
            AstStmtKind::MethodCall(call) => self.method_call_statement(frame, call),
        }
    }

    /// `target := value`: one statement, or, for the assignment of a call of
    /// a function whose result is a structure, the call, the stores of its
    /// outputs and the copy of its result (`structure_result.rs`).
    fn assignment(&mut self, frame: &BodyFrame, assignment: &Assignment) -> Lowered<Vec<Stmt>> {
        // `S=` and `R=` are not lowered yet. `r REF= x` is `r := REF(x)`:
        // the parser writes its value as `REF(x)`.
        if assignment.set_bind || assignment.reset_bind {
            return Err(Unlowered::NotYet);
        }
        if let Some((func, function)) = self.structure_result_call(assignment) {
            return self.structure_result_assignment(frame, assignment, func, function);
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
    ) -> Lowered<Vec<Stmt>> {
        if let Variable::Symbolic(target) = target {
            if crate::bits::selected_bits(target).is_some() {
                let stmt = self.bits_assignment(frame, target, deref, value, span)?;
                return Ok(vec![stmt]);
            }
        }
        let mut place = self.variable_place(frame, target)?;
        if deref {
            place = self.program.deref(place, &target.span())?;
        }
        self.store_at(frame, place, value, span)
    }

    /// Stores the source value `value` at `place`: a string, an aggregate,
    /// or a scalar value narrowed to the place's storage unless it already
    /// fits (REQ-LOW-lowering-046). A selection of strings or aggregates
    /// stores the input it selects (`selection.rs`), after the statements that
    /// evaluate its inputs; any other value is one statement.
    pub(crate) fn store_at(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        match self.program.types().kind(place.ty()) {
            Some(
                TypeKind::String(_)
                | TypeKind::Array { .. }
                | TypeKind::Structure { .. }
                | TypeKind::FunctionBlock { .. },
            ) if self.is_selection(value) => {
                return self.store_selection(frame, place, value, span);
            }
            Some(TypeKind::String(_)) => {
                return Ok(vec![self.string_assignment(frame, place, value, span)?]);
            }
            Some(
                TypeKind::Array { .. }
                | TypeKind::Structure { .. }
                | TypeKind::FunctionBlock { .. },
            ) => return Ok(vec![self.aggregate_assignment(frame, place, value, span)?]),
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
        // A program instance is not assigned.
        let operated = self
            .program
            .types()
            .operation_type(place.ty())
            .ok_or(Unlowered::NotYet)?;
        let value = self.expr(frame, value)?;
        // A reference, an interface value or a value the analyzer did not
        // convert to the target's type is not lowered: the analyzer records
        // every conversion, so a numeric value of another numeric type is a
        // defect, which `Stmt::assign` reports.
        if value.ty() != operated && !(value.ty().is_numeric() && operated.is_numeric()) {
            return Err(Unlowered::NotYet);
        }
        Ok(vec![self.assign_value(place, value, span)?])
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
    /// function block instance (REQ-LOW-lowering-061), `MOVE(x)` and `(x)`
    /// being `x` (REQ-LOW-lowering-203). Any other source is not lowered
    /// here; the assignment of a call of a function that returns a structure
    /// is `structure_result.rs`'s.
    fn aggregate_assignment(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Stmt> {
        let value = self.unwrapped(value);
        let source = if let AstExprKind::CallOutput(output) = &value.kind {
            self.call_output_place(frame, output)?
        } else if let AstExprKind::Variable(variable) = &value.kind {
            self.variable_place(frame, variable)?
        } else {
            return Err(Unlowered::NotYet);
        };
        Ok(self.program.copy(place, source, span)?)
    }
}
