//! The outputs a call stores in their targets (`q => v`), and the
//! [`ExprKind::CallOutput`] each one stores.
//!
//! The analyzer records, on each output assignment of a call whose arguments
//! bind, the value it stores: the output the call leaves, converted to the
//! target's type where they differ (`Output::value`, see
//! `specs/design/call-binding.md`). Code generation compiles it as the
//! assignment of that value to the target. The call output in it is read
//! from wherever the call being compiled leaves its outputs, which only that
//! call knows, so the call says where while its outputs are stored.
//!
//! [`ExprKind::CallOutput`]: ironplc_dsl::textual::ExprKind::CallOutput

use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::ParamAssignmentKind;

use super::compile::CompileContext;
use super::compile_assign::compile_store;
use super::compile_place::Place;
use crate::emit::Emitter;

/// Where the call whose outputs are being stored leaves them.
#[derive(Clone, Debug)]
pub(crate) enum CallOutputs {
    /// The function block instance of this name: an output is its field.
    Instance(Id),
}

/// Stores each output assignment of `params`, the arguments of a call that
/// leaves its outputs where `outputs` says, in its target, after the call.
pub(crate) fn store_outputs(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    outputs: CallOutputs,
    params: &[ParamAssignmentKind],
) -> Result<(), Diagnostic> {
    let enclosing = ctx.call_outputs.replace(outputs);
    let stored = params.iter().try_for_each(|param| {
        let ParamAssignmentKind::Output(output) = param else {
            return Ok(());
        };
        // The binding pass records the value of every output of a call whose
        // arguments bind, and the rules reject any other call.
        let value = output.value.as_ref().ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                output.src.span(),
                "Output of a call the analyzer did not bind",
            ))
        })?;
        compile_store(emitter, ctx, &output.tgt, value)
    });
    ctx.call_outputs = enclosing;
    stored
}

/// Compiles the read of the output `output` of the call whose outputs are
/// being stored, leaving its value on the stack at the output's type.
pub(crate) fn compile_call_output(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    output: &Id,
) -> Result<(), Diagnostic> {
    match ctx.call_outputs.clone() {
        Some(CallOutputs::Instance(instance)) => {
            let place = Place::instance_field(ctx, &instance, output)?.ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    output.span(),
                    format!("Output of '{instance}', which is not a function block instance"),
                ))
            })?;
            place.emit_load(emitter, ctx)
        }
        None => Err(Diagnostic::internal_error_at(Label::span(
            output.span(),
            "Output read outside the call that leaves it",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::Id;

    use super::compile_call_output;
    use crate::compile::CompileContext;
    use crate::emit::Emitter;

    #[test]
    fn compile_call_output_when_outside_a_call_then_internal_error() {
        let mut ctx = CompileContext::new();
        let mut emitter = Emitter::new();

        let result = compile_call_output(&mut emitter, &mut ctx, &Id::from("q"));

        assert_eq!(result.unwrap_err().code, "P9998");
    }
}
