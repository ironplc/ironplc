//! Calls of function blocks.
//!
//! A call `f(x := 1, q => v)` of a function block lowers to three parts, in
//! order (REQ-LOW-lowering-069):
//!
//! 1. the assignment of each input to its field of the instance, in the
//!    order of the call's normal form, which the analyzer wrote
//!    (`specs/design/call-binding.md`): an input the call omits keeps its
//!    value;
//! 2. an `FbCall` of the instance, which runs the block's body;
//! 3. the assignment of each output to its target, of the value the analyzer
//!    recorded for it: the call output `q`, a read of the instance's field
//!    `q`, inside the conversion the analyzer recorded, and `NOT` of it for a
//!    negated output.
//!
//! Each part is the statement the source spells the same way would lower to,
//! so `f(x := 1)` stores `1` as `f.x := 1` does.
//!
//! A call of a standard function block (`TON`, `CTU_DINT`, ...) lowers the
//! same way, to an `FbCall` of the POU named after the block. Lowering does
//! not generate the standard blocks yet (issue 2288), so that POU has no body
//! in the lowered program, and the bytecode backend runs the VM's own block
//! on the instance. Until then, such an `FbCall` names a standard function
//! block, which REQ-LOW-lowering-077 rules out once lowering generates them
//! (ADR-0060).
//!
//! An output of a structure, array or function block type is copied to its
//! target, as `p := f.o` copies it.
//!
//! Not lowered yet: a call that binds a `VAR_IN_OUT`, and a call with an
//! input or output lowering does not store yet.

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{FbCall, ParamAssignmentKind};
use ironplc_ir::lowered::{Expr, Place, Section, Stmt};

use crate::variables::{BodyFrame, CallOutputs};
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// The call `call` of a function block instance, in the body of `frame`.
    pub(crate) fn fb_call(&mut self, frame: &BodyFrame, call: &FbCall) -> Lowered<Vec<Stmt>> {
        // Every part is the statement the call is, for the line map
        // (REQ-LOW-lowering-063).
        let span = call.span();
        let instance = self.named_place(frame, &call.var_name, &call.var_name.span())?;
        let mut stmts = Vec::with_capacity(call.params.len() + 1);
        for param in &call.params {
            let input = match param {
                ParamAssignmentKind::NamedInput(input) => input,
                // The binding pass names every input of a call that binds,
                // and the rules reject one that does not.
                ParamAssignmentKind::PositionalInput(input) => {
                    return Err(Diagnostic::internal_error_at(Label::span(
                        input.expr.span(),
                        "Positional input of a function block call the analyzer did not bind",
                    ))
                    .into())
                }
                ParamAssignmentKind::Output(_) => continue,
            };
            let field = self.block_field(&instance, &input.name, Section::Input)?;
            stmts.extend(self.store_at(frame, field, &input.expr, span.clone())?);
        }
        stmts.push(self.program.fb_call(instance.clone(), span.clone())?);
        stmts.extend(self.outputs(frame, CallOutputs::Instance(instance), &call.params, &span)?);
        Ok(stmts)
    }

    /// The assignment of each output of `params` to its target, of the value
    /// the analyzer recorded for it, where the call whose outputs they are
    /// left them in `left`, which [`Self::call_output`] reads.
    pub(crate) fn outputs(
        &mut self,
        frame: &BodyFrame,
        left: CallOutputs,
        params: &[ParamAssignmentKind],
        span: &SourceSpan,
    ) -> Lowered<Vec<Stmt>> {
        let storing = frame.storing(&left);
        let mut stmts = Vec::new();
        for param in params {
            let ParamAssignmentKind::Output(output) = param else {
                continue;
            };
            // The binding pass records the value of every output of a call
            // whose arguments bind, and the rules reject any other call.
            let value = output.value.as_ref().ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    output.src.span(),
                    "Output of a call the analyzer did not bind",
                ))
            })?;
            stmts.extend(self.store(&storing, &output.tgt, false, value, span.clone())?);
        }
        Ok(stmts)
    }

    /// The call output `output`, the expression whose span is `span`, in a
    /// statement of `frame` that stores the outputs of a call: a read of
    /// where that call left it.
    pub(crate) fn call_output(
        &mut self,
        frame: &BodyFrame,
        output: &Id,
        span: SourceSpan,
    ) -> Lowered<Expr> {
        let place = self.call_output_place(frame, output)?;
        self.read(place, span)
    }

    /// Where the call whose outputs a statement of `frame` stores left its
    /// output `output`: the field `output` of a function block's instance,
    /// or the temporary a function or method call passed for it. A name that
    /// is not one of its outputs, such as `ENO`, is not lowered.
    pub(crate) fn call_output_place(&mut self, frame: &BodyFrame, output: &Id) -> Lowered<Place> {
        match frame.call_outputs() {
            Some(CallOutputs::Instance(instance)) => {
                self.block_field(instance, output, Section::Output)
            }
            Some(CallOutputs::Temporaries(temporaries)) => temporaries
                .iter()
                .find_map(|(name, temporary)| (name == output).then(|| temporary.clone()))
                .ok_or(Unlowered::NotYet),
            // A call output is written only on an output assignment, which
            // is lowered only with the outputs of its call.
            None => Err(Unlowered::NotYet),
        }
    }

    /// The field `name` of the function block instance `instance`, when it
    /// is one of `section`. Any other field, such as a `VAR_IN_OUT`, and a
    /// field the block's lowered type does not hold, is not lowered.
    fn block_field(&mut self, instance: &Place, name: &Id, section: Section) -> Lowered<Place> {
        let (field, declared) = self.field_named(instance, name).ok_or(Unlowered::NotYet)?;
        if declared.section != Some(section) {
            return Err(Unlowered::NotYet);
        }
        Ok(self.program.field(instance.clone(), field, &name.span())?)
    }
}
