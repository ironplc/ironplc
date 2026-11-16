//! `SEL` and `MUX` of strings, arrays, structures and function block
//! instances, and `MIN`, `MAX` and `LIMIT` of strings, stored at a place
//! (REQ-LOW-lowering-204, REQ-LOW-lowering-208).
//!
//! A string is a `StrExpr` and an aggregate a place, and neither has an
//! expression that selects, so a selection of one lowers where it is stored:
//! to the `If` (`SEL`) or `Case` (`MUX`) whose arms store the input each one
//! selects. The selection evaluates its selector and then every input, in the
//! order of its parameters, as a call of any function does and as `SEL` and
//! `MUX` of a number do: an input that is not selected still runs its calls,
//! and still traps on a subscript out of range.
//!
//! `MIN`, `MAX` and `LIMIT` of strings select one of their inputs too, by the
//! order of the inputs rather than by a selector: they lower to the chain of
//! `If`s whose conditions compare the inputs, as a comparison of strings
//! does, and whose arms store the input each selects. Every condition is
//! evaluated before any arm stores, so the target may be one of the inputs.
//!
//! When every input is a literal, or a variable reached through fields and
//! constant subscripts only, evaluating an input has no effect and cannot
//! trap, and its value is the one it had when the selector was evaluated. So
//! each arm stores its input directly, and each condition compares the
//! inputs themselves. Otherwise the selector, unless it is a constant, and
//! then each input but a literal are stored in temporaries of their own
//! types, in order, and each arm stores, and each condition compares, its
//! temporary.
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
    Arg, BinaryOp, CaseArm, CaseLabel, CompareOp, Const, Expr, ExprKind, Nullability, Place,
    Projection, Stmt, StrExpr, TypeKind,
};

use crate::string::{compared_order, order_intrinsic};
use crate::variables::BodyFrame;
use crate::{Lowered, Lowering, Unlowered};

/// The selection function a call is.
#[derive(Clone, Copy)]
enum Selection {
    /// `SEL` or `MUX`, which select by the value of their first input.
    By(Selector),
    /// `MIN`, `MAX` or `LIMIT`, which select by the order of their inputs.
    Ordered(Ordered),
}

/// A selection function that selects by the value of its first input.
#[derive(Clone, Copy)]
enum Selector {
    /// `SEL(G, IN0, IN1)`, which selects by a `BOOL`.
    Sel,
    /// `MUX(K, IN0, ...)`, which selects by an integer, clamped to its inputs.
    Mux,
}

/// A selection function that selects by the order of its inputs.
#[derive(Clone, Copy)]
enum Ordered {
    /// `MIN(IN1, IN2, ...)`, which selects its least input.
    Min,
    /// `MAX(IN1, IN2, ...)`, which selects its greatest input.
    Max,
    /// `LIMIT(MN, IN, MX)`, which selects `IN` limited to `MN` and `MX`.
    Limit,
}

impl Selector {
    /// The name of the parameter that selects.
    fn name(self) -> &'static str {
        match self {
            Selector::Sel => "G",
            Selector::Mux => "K",
        }
    }
}

impl Selection {
    /// The name of the parameter of the input at `index` among the inputs
    /// the function selects between.
    fn input(self, index: usize) -> String {
        match self {
            Selection::By(Selector::Sel | Selector::Mux) => format!("IN{index}"),
            Selection::Ordered(Ordered::Min | Ordered::Max) => format!("IN{}", index + 1),
            Selection::Ordered(Ordered::Limit) => ["MN", "IN", "MX"]
                .get(index)
                .map_or_else(|| format!("IN{index}"), |name| (*name).to_string()),
        }
    }
}

/// Where an arm of a selection stores its input from.
#[derive(Clone)]
enum Source<'e> {
    /// The input itself, evaluated in the arm.
    Input(&'e AstExpr),
    /// The temporary the input was stored in before the selection.
    Temporary(Place),
}

impl Lowering<'_> {
    /// Whether `value` is a call of `SEL`, `MUX`, `MIN`, `MAX` or `LIMIT`,
    /// through parentheses and `MOVE`.
    pub(crate) fn is_selection(&self, value: &AstExpr) -> bool {
        self.selection_call(value).is_some()
    }

    /// The call of a selection function that `value` is, through parentheses
    /// and `MOVE`, and which one; `None` for any other value.
    fn selection_call<'e>(&self, value: &'e AstExpr) -> Option<(Selection, &'e Function)> {
        let AstExprKind::Function(func) = &self.unwrapped(value).kind else {
            return None;
        };
        let signature = self.analysis.context().functions().get(&func.name)?;
        let selection = match signature.builtin_function.as_ref()? {
            BuiltinFunction::Mux => Selection::By(Selector::Mux),
            BuiltinFunction::Numeric(function) => numeric_selection(*function)?,
            BuiltinFunction::Operator(_)
            | BuiltinFunction::BitShift(_)
            | BuiltinFunction::Move
            | BuiltinFunction::Trunc
            | BuiltinFunction::BcdToInt
            | BuiltinFunction::IntToBcd
            | BuiltinFunction::Sizeof
            | BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => return None,
        };
        Some((selection, func))
    }

    /// Stores at `place`, a string or an aggregate, the input the selection
    /// `value` selects, after evaluating its selector, if it has one, and
    /// every input in order (REQ-LOW-lowering-204, REQ-LOW-lowering-208).
    pub(crate) fn store_selection(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        value: &AstExpr,
        span: SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        let (selection, func) = self.selection_call(value).ok_or(Unlowered::NotYet)?;
        // The analyzer wrote the call in normal form: its inputs in the order
        // of its parameters, the selector of `SEL` and `MUX` first.
        let args: Vec<&AstExpr> = func.inputs().collect();
        let name = func.name.original();
        match selection {
            Selection::By(selector) => {
                self.store_selected_by(frame, place, selector, name, &args, span)
            }
            Selection::Ordered(ordered) => {
                self.store_ordered(frame, place, ordered, name, &args, span)
            }
        }
    }

    /// Stores at `place` the input `SEL` or `MUX` of `args` selects by its
    /// selector, the first of `args` (REQ-LOW-lowering-204).
    fn store_selected_by(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        function: Selector,
        name: &str,
        args: &[&AstExpr],
        span: SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        let (selector, inputs) = args.split_first().ok_or(Unlowered::NotYet)?;
        if inputs.len() < 2 {
            return Err(Unlowered::NotYet);
        }
        let mut stmts = Vec::new();
        let mut selected_by = self.expr(frame, selector)?;
        let settled = inputs.iter().all(|input| self.is_settled(frame, input));
        if !settled && !matches!(selected_by.kind(), ExprKind::Const(_)) {
            let ty = self.typed(selector)?.ty;
            let label = format!("{name}.{}", function.name());
            let temporary = self.temporary(frame, label, ty, &span)?;
            stmts.push(self.assign_value(temporary.clone(), selected_by, span.clone())?);
            selected_by = self.program.read(temporary, span.clone())?;
        }
        let by = Selection::By(function);
        let (stored, sources) = self.sources(frame, by, name, inputs, settled, &span)?;
        stmts.extend(stored);
        let mut arms = Vec::with_capacity(sources.len());
        for source in sources {
            arms.push(self.store_source(frame, &place, source, &span)?);
        }
        stmts.push(match function {
            Selector::Sel => {
                let [in0, in1]: [Vec<Stmt>; 2] = arms.try_into().map_err(|_| Unlowered::NotYet)?;
                Stmt::if_(selected_by, in1, in0, span)?
            }
            // The selector is clamped to the inputs: one below the first
            // selects the first, and one past the last the last.
            Selector::Mux => {
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

    /// Stores at `place`, a string, the input `MIN`, `MAX` or `LIMIT` of
    /// strings selects by the order of its `inputs` (REQ-LOW-lowering-208):
    /// the chain of `If`s on the conditions [`Self::ordered_arms`] gives,
    /// each of whose arms stores the input it selects, after the stores of
    /// the inputs that need them in temporaries.
    fn store_ordered(
        &mut self,
        frame: &BodyFrame,
        place: Place,
        ordered: Ordered,
        name: &str,
        inputs: &[&AstExpr],
        span: SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        if inputs.len() < 2 {
            return Err(Unlowered::NotYet);
        }
        let settled = inputs.iter().all(|input| self.is_settled(frame, input));
        let selection = Selection::Ordered(ordered);
        let (mut stmts, sources) = self.sources(frame, selection, name, inputs, settled, &span)?;
        let (arms, otherwise) = self.ordered_arms(frame, ordered, sources, &span)?;
        let mut chain = self.store_source(frame, &place, otherwise, &span)?;
        for (condition, source) in arms.into_iter().rev() {
            let selected = self.store_source(frame, &place, source, &span)?;
            chain = vec![Stmt::if_(condition, selected, chain, span.clone())?];
        }
        stmts.extend(chain);
        Ok(stmts)
    }

    /// Each condition on which `ordered` selects one of `sources`, in order,
    /// with the input it selects, and the input it selects when none holds.
    ///
    /// `MIN` selects the first input that is at most every later one, and
    /// `MAX` the first that is at least every later one; else the last. An
    /// earlier input is not selected only when a later one is less (or
    /// greater), so the input selected is the least (or greatest) of all,
    /// as the two-input function folded from the left is.
    ///
    /// `LIMIT(MN, IN, MX)`, which is `MIN(MAX(IN, MN), MX)`, selects `MX`
    /// when it is less than `IN` or than `MN`, else `MN` when `IN` is less
    /// than it, else `IN`: `MX` when `MN` is above it, as for a number.
    fn ordered_arms<'e>(
        &mut self,
        frame: &BodyFrame,
        ordered: Ordered,
        mut sources: Vec<Source<'e>>,
        span: &SourceSpan,
    ) -> Lowered<(Vec<(Expr, Source<'e>)>, Source<'e>)> {
        let op = match ordered {
            Ordered::Min => CompareOp::Le,
            Ordered::Max => CompareOp::Ge,
            Ordered::Limit => {
                let [mn, input, mx]: [Source<'e>; 3] =
                    sources.try_into().map_err(|_| Unlowered::NotYet)?;
                let mx_below_input =
                    self.compare_sources(frame, CompareOp::Lt, &mx, &input, span)?;
                let mx_below_mn = self.compare_sources(frame, CompareOp::Lt, &mx, &mn, span)?;
                let limited =
                    Expr::binary(BinaryOp::BoolOr, mx_below_input, mx_below_mn, span.clone())?;
                let input_below_mn =
                    self.compare_sources(frame, CompareOp::Lt, &input, &mn, span)?;
                return Ok((vec![(limited, mx), (input_below_mn, mn)], input));
            }
        };
        let last = sources.pop().ok_or(Unlowered::NotYet)?;
        let mut arms = Vec::with_capacity(sources.len());
        for (index, source) in sources.iter().enumerate() {
            let mut later = sources[index + 1..].iter().chain(std::iter::once(&last));
            let first = later.next().ok_or(Unlowered::NotYet)?;
            let mut condition = self.compare_sources(frame, op, source, first, span)?;
            for other in later {
                let compared = self.compare_sources(frame, op, source, other, span)?;
                condition = Expr::binary(BinaryOp::BoolAnd, condition, compared, span.clone())?;
            }
            arms.push((condition, source.clone()));
        }
        Ok((arms, last))
    }

    /// The comparison `op` of the strings `left` and `right` hold, as a
    /// comparison of strings is lowered (REQ-LOW-lowering-169).
    fn compare_sources(
        &mut self,
        frame: &BodyFrame,
        op: CompareOp,
        left: &Source<'_>,
        right: &Source<'_>,
        span: &SourceSpan,
    ) -> Lowered<Expr> {
        let left = self.source_string(frame, left, span)?;
        let right = self.source_string(frame, right, span)?;
        let intrinsic = order_intrinsic(left.shape().encoding);
        compared_order(
            intrinsic,
            op,
            vec![Arg::Str(left), Arg::Str(right)],
            span.clone(),
        )
    }

    /// The string `source` holds: its input, or a read of its temporary.
    fn source_string(
        &mut self,
        frame: &BodyFrame,
        source: &Source<'_>,
        span: &SourceSpan,
    ) -> Lowered<StrExpr> {
        match source {
            Source::Input(input) => self.str_expr(frame, input),
            Source::Temporary(temporary) => {
                Ok(self.program.read_str(temporary.clone(), span.clone())?)
            }
        }
    }

    /// Where the arms of the selection `name` store each of `inputs` from,
    /// and the statements to run before them. When every input is
    /// `settled`, each arm stores its input itself, and nothing runs first.
    /// Otherwise each input but a literal is stored, in order, in a
    /// temporary of its type, and its arm stores the temporary.
    fn sources<'e>(
        &mut self,
        frame: &BodyFrame,
        selection: Selection,
        name: &str,
        inputs: &[&'e AstExpr],
        settled: bool,
        span: &SourceSpan,
    ) -> Lowered<(Vec<Stmt>, Vec<Source<'e>>)> {
        let mut stmts = Vec::new();
        let mut sources = Vec::with_capacity(inputs.len());
        for (index, input) in inputs.iter().enumerate() {
            if settled || matches!(self.unwrapped(input).kind, AstExprKind::Const(_)) {
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
            let label = format!("{name}.{}", selection.input(index));
            let temporary = self.temporary(frame, label, ty, span)?;
            stmts.extend(self.store_at(frame, temporary.clone(), input, span.clone())?);
            sources.push(Source::Temporary(temporary));
        }
        Ok((stmts, sources))
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

/// The selection function the numeric function `function` is; `None` for one
/// that computes rather than selects.
fn numeric_selection(function: NumericFunction) -> Option<Selection> {
    match function {
        NumericFunction::Sel => Some(Selection::By(Selector::Sel)),
        NumericFunction::Min => Some(Selection::Ordered(Ordered::Min)),
        NumericFunction::Max => Some(Selection::Ordered(Ordered::Max)),
        NumericFunction::Limit => Some(Selection::Ordered(Ordered::Limit)),
        NumericFunction::Abs
        | NumericFunction::Sqrt
        | NumericFunction::Ln
        | NumericFunction::Log
        | NumericFunction::Exp
        | NumericFunction::Sin
        | NumericFunction::Cos
        | NumericFunction::Tan
        | NumericFunction::Asin
        | NumericFunction::Acos
        | NumericFunction::Atan
        | NumericFunction::Atan2
        | NumericFunction::TruncReal
        | NumericFunction::ModReal
        | NumericFunction::Expt => None,
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
