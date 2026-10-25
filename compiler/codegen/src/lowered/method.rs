//! Emits a lowered call of a method.
//!
//! A method's call passes a `Ref` to the receiver first, for the method's
//! instance parameter (REQ-LOW-lowering-076). This backend emits the
//! receiver's instance pushed with `FB_LOAD_INSTANCE`, each input, then
//! `METHOD_CALL`, which copies the instance's fields in and out around the
//! body, as "One field model per body" requires until issue 2120 lands. The
//! call leaves the instance on the stack beneath the result, if any. A
//! method leaves its outputs in its own variables, as a function does.
//!
//! The backend reaches the receiver as a variable the layout keeps, and
//! finds the method in its layout by the names of its block and itself. A method a block inherits is
//! the method of the block that declares it.
//!
//! `METHOD_CALL` names how many fields it copies by a byte. The layout
//! reports a block with more fields than that as P9997 (REQ-LOW-codegen-191),
//! so every block a call reaches fits.

use std::collections::HashMap;

use ironplc_container::{FunctionId, VarIndex};
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{Arg, PouId, VarId};
use ironplc_ir::Diagnostic;
use ironplc_ir::SourceSpan;

use super::call_args::{emit_passed, passes};
use super::place::variable_name;
use crate::compile::{CompileContext, ParamPassing, StringVarInfo};
use crate::emit::Emitter;

/// A call of a method, as this backend's layout knows it: the receiver's
/// instance, and the method.
pub(super) struct MethodSite {
    /// The slot of the receiver's instance.
    instance: VarIndex,
    function_id: FunctionId,
    /// Where the method's block's fields start, and how many there are.
    field_var_off: VarIndex,
    num_fields: u8,
    /// Where the method's own parameters start, and how many it has.
    param_var_off: VarIndex,
    num_params: u16,
    pub(super) has_return_value: bool,
    /// How a call passes each input, in order.
    params: Vec<ParamPassing>,
    /// The slot of each output.
    pub(super) outputs: HashMap<VarId, VarIndex>,
    /// Where each string output is in the data region.
    pub(super) string_outputs: HashMap<VarId, StringVarInfo>,
}

impl MethodSite {
    /// Whether `inputs` are one for each of the method's inputs, each passed
    /// as its parameter takes it. A call of a method with a `VAR_IN_OUT` is
    /// not supported yet (P9999).
    pub(super) fn takes<'s>(&self, inputs: impl ExactSizeIterator<Item = &'s Arg>) -> bool {
        let in_out = self
            .params
            .iter()
            .any(|param| matches!(param, ParamPassing::Reference));
        !in_out && passes(&self.params, inputs)
    }
}

/// The call of the method `pou` with `args`, whose first is a `Ref` to the
/// receiver: an instance this backend reaches as a variable, of the block whose
/// method `pou` is or of a block that extends it. `None` when `pou` is no
/// method, or this backend does not reach the receiver or the method.
pub(super) fn method_site(
    lowering: &Program,
    ctx: &CompileContext,
    pou: PouId,
    args: &[Arg],
) -> Option<MethodSite> {
    let block = ctx.user_fb_type(lowering.pous().method_of(pou)?)?;
    let method = block.methods.get(&pou)?;
    let Some(Arg::Ref(receiver)) = args.first() else {
        return None;
    };
    let instance = ctx.fb_instance(&variable_name(lowering, ctx, receiver)?)?;
    // The lowered call passes an instance of a block that extends the
    // method's (REQ-LOW-ir-176). This backend's layout of a derived block
    // holds none of the fields it inherits, so the method runs on it only
    // when its block declares none (REQ-LOW-codegen-178), and `METHOD_CALL`
    // then copies no field.
    if instance.type_id != block.type_id && block.num_fields > 0 {
        return None;
    }
    Some(MethodSite {
        instance: instance.var_index,
        function_id: method.function_id,
        field_var_off: VarIndex::new(block.var_offset),
        num_fields: u8::try_from(block.num_fields).ok()?,
        param_var_off: VarIndex::new(method.param_var_off),
        num_params: method.num_params,
        has_return_value: method.has_return_value,
        params: method.params.clone(),
        outputs: method.outputs.clone(),
        string_outputs: method.string_outputs.clone(),
    })
}

/// Emits the call `site` with `inputs`, at `span`: the instance, each
/// input, then `METHOD_CALL`, which leaves the instance on the stack beneath
/// the result, if any.
pub(super) fn emit_method_call<'s>(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    site: &MethodSite,
    inputs: impl IntoIterator<Item = &'s Arg>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    emitter.emit_fb_load_instance(site.instance);
    emit_passed(emitter, ctx, lowering, &site.params, inputs, span)?;
    ctx.record_call_edge(site.function_id);
    let stack_depth = ctx.function_stack_depth(site.function_id, span)?;
    emitter.emit_method_call(
        site.function_id,
        site.field_var_off,
        site.num_fields,
        site.param_var_off,
        site.num_params,
        site.has_return_value,
        stack_depth,
    );
    Ok(())
}
