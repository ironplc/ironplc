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
//! A function block leaves its outputs in its instance's fields, and a
//! function or method in its own output variables, in its region of the
//! variable table (ADR-0046). A function or method does not recurse, so no
//! other call of it changes those variables before the caller reads them.
//!
//! [`ExprKind::CallOutput`]: ironplc_dsl::textual::ExprKind::CallOutput

use std::collections::HashMap;

use ironplc_container::VarIndex;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ParamAssignmentKind};

use super::compile::CompileContext;
use super::compile_assign::compile_store;
use super::compile_expr::{emit_load_var, op_type_from_expr};
use super::compile_place::Place;
use crate::emit::Emitter;

/// Where the call whose outputs are being stored leaves them.
#[derive(Clone, Debug)]
pub(crate) enum CallOutputs {
    /// The function block instance of this name: an output is its field.
    Instance(Id),
    /// The output variables of the function or method called: each output's
    /// slot in its region of the variable table.
    Variables(HashMap<Id, VarIndex>),
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

/// Compiles `read`, the read of the output `output` of the call whose
/// outputs are being stored, leaving its value on the stack at the output's
/// type.
pub(crate) fn compile_call_output(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    read: &Expr,
    output: &Id,
) -> Result<(), Diagnostic> {
    match ctx.call_outputs.clone() {
        Some(CallOutputs::Variables(variables)) => {
            let var_index = variables.get(output).copied().ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    output.span(),
                    "Output the called function or method does not declare",
                ))
            })?;
            // A string or an aggregate output is not one slot's value.
            let at = op_type_from_expr(ctx, read).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    output.span(),
                    "Output of a function or method that is not a single value",
                ))
            })?;
            emit_load_var(emitter, var_index, at);
            Ok(())
        }
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
    use std::collections::HashMap;

    use ironplc_dsl::core::Id;
    use ironplc_dsl::textual::{Expr, ExprKind};

    use super::{compile_call_output, CallOutputs};
    use crate::compile::CompileContext;
    use crate::emit::Emitter;

    fn read(output: &str) -> Expr {
        Expr::new(ExprKind::CallOutput(Id::from(output)))
    }

    #[test]
    fn compile_call_output_when_outside_a_call_then_internal_error() {
        let mut ctx = CompileContext::new();
        let mut emitter = Emitter::new();

        let result = compile_call_output(&mut emitter, &mut ctx, &read("q"), &Id::from("q"));

        assert_eq!(result.unwrap_err().code, "P9998");
    }

    #[test]
    fn compile_call_output_when_callee_declares_no_such_output_then_internal_error() {
        let mut ctx = CompileContext::new();
        ctx.call_outputs = Some(CallOutputs::Variables(HashMap::new()));
        let mut emitter = Emitter::new();

        let result = compile_call_output(&mut emitter, &mut ctx, &read("q"), &Id::from("q"));

        assert_eq!(result.unwrap_err().code, "P9998");
    }
}
