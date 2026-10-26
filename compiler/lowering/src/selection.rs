//! `SEL` and `MUX` of strings, arrays, structures and function block
//! instances, stored at a place (REQ-LOW-lowering-204).
//!
//! A string is a `StrExpr` and an aggregate a place, and neither has an
//! expression that selects, so a selection of one lowers where it is stored:
//! to the `If` (`SEL`) or `Case` (`MUX`) whose arms store the input each one
//! selects. The selection evaluates its selector and then every input, in the
//! order of its parameters, as a call of any function does and as `SEL` and
//! `MUX` of a number do: an input that is not selected still runs its calls,
//! and still traps on a subscript out of range.
//!
//! When every input is a literal, or a variable reached through fields and
//! constant subscripts only, evaluating an input has no effect and cannot
//! trap, and its value is the one it had when the selector was evaluated. So
//! each arm stores its input directly. Otherwise the selector, unless it is a
//! constant, and then each input but a literal are stored in temporaries of
//! their own types, in order, and each arm stores its temporary.
//!
//! A selection used inside an expression, such as an argument of `CONCAT` or
//! of a function, is not lowered yet, and neither is an input that is a call
//! of a function whose result is a structure.

use ironplc_analyzer::{BuiltinFunction, NumericFunction};
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::textual::{
    Expr as AstExpr, ExprKind as AstExprKind, Function, SymbolicVariableKind, Variable,
};
use ironplc_ir::lowered::{
    CaseArm, CaseLabel, Const, ExprKind, Nullability, Place, Projection, Stmt, TypeKind,
};

use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

/// The selection function a call is.
#[derive(Clone, Copy)]
enum Selection {
    /// `SEL(G, IN0, IN1)`, which selects by a `BOOL`.
    Sel,
    /// `MUX(K, IN0, ...)`, which selects by an integer, clamped to its inputs.
    Mux,
}

impl Selection {
    /// The name of the parameter that selects.
    fn selector(self) -> &'static str {
        match self {
            Selection::Sel => "G",
            Selection::Mux => "K",
        }
    }
}

/// Where an arm of a selection stores its input from.
enum Source<'e> {
    /// The input itself, evaluated in the arm.
    Input(&'e AstExpr),
    /// The temporary the input was stored in before the selection.
    Temporary(Place),
}

impl Lowering<'_> {
    /// Whether `value` is a call of `SEL` or `MUX`, through parentheses and
    /// `MOVE`.
    pub(crate) fn is_selection(&self, value: &AstExpr) -> bool {
        self.selection_call(value).is_some()
    }

    /// The call of `SEL` or `MUX` that `value` is, through parentheses and
    /// `MOVE`, and which of the two; `None` for any other value.
    fn selection_call<'e>(&self, value: &'e AstExpr) -> Option<(Selection, &'e Function)> {
        let AstExprKind::Function(func) = &self.unwrapped(value).kind else {
            return None;
        };
        let signature = self.analysis.context().functions().get(&func.name)?;
        if matches!(
            signature.builtin_function,
            Some(BuiltinFunction::Numeric(NumericFunction::Sel))
        ) {
            Some((Selection::Sel, func))
        } else if matches!(signature.builtin_function, Some(BuiltinFunction::Mux)) {
            Some((Selection::Mux, func))
        } else {
            None
        }
    }

    /// Stores at `place`, a string or an aggregate, the input the selection
    /// `value` selects, after evaluating its selector and every input in order
    /// (REQ-LOW-lowering-204).
    pub(crate) fn store_selection(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        let (selection, func) = self.selection_call(value).ok_or(Unlowered::NotYet)?;
        // The analyzer wrote the call in normal form: its selector, then its
        // inputs, in the order of its parameters.
        let args: Vec<&AstExpr> = func.inputs().collect();
        let (selector, inputs) = args.split_first().ok_or(Unlowered::NotYet)?;
        if inputs.len() < 2 {
            return Err(Unlowered::NotYet);
        }
        let name = func.name.original();
        let mut stmts = Vec::new();
        let mut selected_by = self.expr(frame, selector)?;
        let mut sources = Vec::with_capacity(inputs.len());
        if inputs.iter().all(|input| self.is_settled(frame, input)) {
            sources.extend(inputs.iter().map(|input| Source::Input(input)));
        } else {
            if !matches!(selected_by.kind(), ExprKind::Const(_)) {
                let ty = self.typed(selector)?.ty;
                let label = format!("{name}.{}", selection.selector());
                let temporary = self.temporary(frame, label, ty, &span)?;
                stmts.push(self.assign_value(temporary.clone(), selected_by, span.clone())?);
                selected_by = self.program.read(temporary, span.clone())?;
            }
            for (index, input) in inputs.iter().enumerate() {
                if matches!(self.unwrapped(input).kind, AstExprKind::Const(_)) {
                    sources.push(Source::Input(input));
                    continue;
                }
                let ty = self.typed(input)?.ty;
                // A function block instance is no temporary of a body.
                if matches!(
                    self.program.types().kind(ty),
                    Some(TypeKind::FunctionBlock { .. })
                ) {
                    return Err(Unlowered::NotYet);
                }
                let temporary = self.temporary(frame, format!("{name}.IN{index}"), ty, &span)?;
                stmts.extend(self.store_at(frame, temporary.clone(), input, span.clone())?);
                sources.push(Source::Temporary(temporary));
            }
        }
        let mut arms = Vec::with_capacity(sources.len());
        for source in sources {
            arms.push(self.store_source(frame, &place, source, &span)?);
        }
        stmts.push(match selection {
            Selection::Sel => {
                let [in0, in1]: [Vec<Stmt>; 2] = arms.try_into().map_err(|_| Unlowered::NotYet)?;
                Stmt::if_(selected_by, in1, in0, span)?
            }
            // The selector is clamped to the inputs: one below the first
            // selects the first, and one past the last the last.
            Selection::Mux => {
                let ty = selected_by.ty();
                let (lowest, _) = ty.integer_range().ok_or(Unlowered::NotYet)?;
                let constant = |value: i128| Const::integer(value, ty).ok_or(Unlowered::NotYet);
                let last = arms.pop().ok_or(Unlowered::NotYet)?;
                let mut labelled = Vec::with_capacity(arms.len());
                for (index, body) in arms.into_iter().enumerate() {
                    let label = if index == 0 {
                        CaseLabel::Range(constant(lowest)?, constant(0)?)
                    } else {
                        CaseLabel::Value(constant(index as i128)?)
                    };
                    labelled.push(CaseArm::new(vec![label], body));
                }
                Stmt::case(selected_by, labelled, last, span)?
            }
        });
        Ok(stmts)
    }

    /// The statements of the arm that stores `source` at `place`.
    fn store_source(
        &mut self,
        frame: &BodyFrame,
        place: &Place,
        source: Source<'_>,
        span: &SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        match source {
            Source::Input(input) => self.store_at(frame, place.clone(), input, span.clone()),
            Source::Temporary(temporary) => {
                let of_string = matches!(
                    self.program.types().kind(place.ty()),
                    Some(TypeKind::String(_))
                );
                let stmt = if of_string {
                    let value = self.program.read_str(temporary, span.clone())?;
                    self.program
                        .assign_str(place.clone(), value, span.clone())?
                } else {
                    self.program.copy(place.clone(), temporary, span.clone())?
                };
                Ok(vec![stmt])
            }
        }
    }

    /// Whether evaluating `input` has no effect and cannot trap, so that its
    /// value is the one it has once the selector is evaluated: a literal, or a
    /// variable reached through fields and constant subscripts only, neither a
    /// reference nor a `VAR_IN_OUT` read through.
    fn is_settled(&mut self, frame: &BodyFrame, input: &AstExpr) -> bool {
        let input = self.unwrapped(input);
        if let AstExprKind::Const(_) = &input.kind {
            return true;
        }
        let AstExprKind::Variable(variable) = &input.kind else {
            return false;
        };
        let Variable::Symbolic(kind) = variable else {
            return false;
        };
        // The subscripts are checked before the place is lowered, so that
        // lowering one does not lower a call it holds.
        if !reached_through_constants(kind) {
            return false;
        }
        self.variable_place(frame, variable)
            .is_ok_and(|place| self.reached_without_traps(&place))
    }

    /// Whether reaching `place` computes no subscript and goes through no
    /// reference that may be null: through fields, constant subscripts, and
    /// references that are never null, such as the instance of the body and
    /// a `VAR_IN_OUT`.
    fn reached_without_traps(&self, place: &Place) -> bool {
        let types = self.program.types();
        let Some(mut ty) = self
            .program
            .variables()
            .get(place.root())
            .map(|variable| variable.ty())
        else {
            return false;
        };
        for step in place.path() {
            let next = match step {
                Projection::Field(field) => types
                    .fields(ty)
                    .and_then(|fields| fields.get(field.index() as usize))
                    .map(|declared| declared.ty),
                Projection::Index(subscripts) => {
                    let constant = subscripts
                        .iter()
                        .all(|subscript| matches!(subscript.kind(), ExprKind::Const(_)));
                    match types.kind(ty) {
                        Some(TypeKind::Array { element, .. }) if constant => Some(*element),
                        Some(_) | None => None,
                    }
                }
                Projection::Deref => match types.reference(ty) {
                    Some((target, Nullability::NonNull)) => Some(target),
                    Some((_, Nullability::Nullable)) | None => None,
                },
            };
            let Some(next) = next else {
                return false;
            };
            ty = next;
        }
        true
    }
}

/// Whether the variable `kind` is reached through fields and constant
/// subscripts only.
fn reached_through_constants(kind: &SymbolicVariableKind) -> bool {
    match kind {
        SymbolicVariableKind::Named(_) => true,
        SymbolicVariableKind::Structured(structured) => {
            reached_through_constants(&structured.record)
        }
        SymbolicVariableKind::Array(array) => {
            reached_through_constants(&array.subscripted_variable)
                && array
                    .subscripts
                    .iter()
                    .all(|subscript| matches!(subscript.kind, AstExprKind::Const(_)))
        }
        SymbolicVariableKind::Deref(_)
        | SymbolicVariableKind::SelfRef(_)
        | SymbolicVariableKind::BitAccess(_)
        | SymbolicVariableKind::PartialAccess(_) => false,
    }
}
