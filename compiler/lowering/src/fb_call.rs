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
use ironplc_ir::lowered::{Place, Section, Stmt};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::{CallOutputs, Lowering};

/// A lowered node, `None` when lowering does not support it, or something
/// inside it, yet.
type Made<T> = Result<Option<T>, Diagnostic>;

impl Lowering<'_> {
    /// The call `call` of a function block instance, in the body of `frame`.
    pub(crate) fn fb_call(&mut self, frame: &BodyFrame, call: &FbCall) -> Made<Vec<Stmt>> {
        // Every part is the statement the call is, for the line map
        // (REQ-LOW-lowering-063).
        let span = call.span();
        let Some(instance) = self.named_place(frame, &call.var_name, &call.var_name.span())? else {
            return Ok(None);
        };
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
                    )))
                }
                ParamAssignmentKind::Output(_) => continue,
            };
            let Some(field) = self.block_field(&instance, &input.name, Section::Input)? else {
                return Ok(None);
            };
            let Some(stmt) = self.store_at(frame, field, &input.expr, span.clone())? else {
                return Ok(None);
            };
            stmts.push(stmt);
        }
        stmts.push(self.program.fb_call(instance.clone(), span.clone())?);
        let Some(outputs) =
            self.outputs(frame, CallOutputs::Instance(instance), &call.params, &span)?
        else {
            return Ok(None);
        };
        stmts.extend(outputs);
        Ok(Some(stmts))
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
    ) -> Made<Vec<Stmt>> {
        let enclosing = self.call_outputs.replace(left);
        let stmts = self.output_stores(frame, params, span);
        self.call_outputs = enclosing;
        stmts
    }

    /// The assignment of each output of `params` to its target.
    fn output_stores(
        &mut self,
        frame: &BodyFrame,
        params: &[ParamAssignmentKind],
        span: &SourceSpan,
    ) -> Made<Vec<Stmt>> {
        let mut stmts = Vec::new();
        for param in params {
            let ParamAssignmentKind::Output(output) = param else {
                continue;
            };
            // The binding pass records the value of every output of a call
            // whose arguments bind, and the rules reject any other call.
            let Some(value) = &output.value else {
                return Err(Diagnostic::internal_error_at(Label::span(
                    output.src.span(),
                    "Output of a call the analyzer did not bind",
                )));
            };
            let Some(stmt) = self.store(frame, &output.tgt, false, value, span.clone())? else {
                return Ok(None);
            };
            stmts.push(stmt);
        }
        Ok(Some(stmts))
    }

    /// The call output `output`, the expression whose span is `span`: a read
    /// of where the call whose outputs are being lowered left it.
    pub(crate) fn call_output(&mut self, output: &Id, span: SourceSpan) -> Lowered {
        let Some(place) = self.call_output_place(output)? else {
            return Ok(None);
        };
        self.read(place, span)
    }

    /// Where the call whose outputs are being lowered left its output
    /// `output`: the field `output` of a function block's instance, or the
    /// temporary a function or method call passed for it. `None` for a name
    /// that is not one of its outputs, such as `ENO`.
    pub(crate) fn call_output_place(&mut self, output: &Id) -> Made<Place> {
        // A call output is written only on an output assignment, which is
        // lowered only with the outputs of its call.
        match self.call_outputs.clone() {
            Some(CallOutputs::Instance(instance)) => {
                self.block_field(&instance, output, Section::Output)
            }
            Some(CallOutputs::Temporaries(temporaries)) => Ok(temporaries
                .into_iter()
                .find_map(|(name, temporary)| (name == *output).then_some(temporary))),
            None => Ok(None),
        }
    }

    /// The field `name` of the function block instance `instance`, when it
    /// is one of `section`; `None` for any other field, such as a
    /// `VAR_IN_OUT`, and for a field the block's lowered type does not hold.
    fn block_field(&mut self, instance: &Place, name: &Id, section: Section) -> Made<Place> {
        let Some(position) = self
            .program
            .types()
            .fields(instance.ty())
            .and_then(|fields| {
                fields.iter().position(|field| {
                    field.section == Some(section) && Id::from(&field.name.to_string()) == *name
                })
            })
        else {
            return Ok(None);
        };
        let Some(field) = self.program.field_idx(instance.ty(), position) else {
            return Ok(None);
        };
        self.program
            .field(instance.clone(), field, &name.span())
            .map(Some)
    }
}
