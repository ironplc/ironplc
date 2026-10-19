//! Compilation of a call to a user-defined function.
//!
//! Separated from `compile_call.rs` to keep module sizes within the
//! 1000-line guideline.

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    Expr, ExprKind, Function, ParamAssignmentKind, SymbolicVariableKind, Variable,
};

use super::call_args::collect_input_args;
use super::compile::{CompileContext, ParamPassing, StringParamInfo, UserFunctionInfo};
use super::compile_call_output::{store_outputs, CallOutputs};
use super::compile_expr::compile_expr;
use super::compile_string::resolve_string_arg;
use crate::emit::Emitter;

/// Compiles a call to a user-defined function.
///
/// For STRING parameters, copies the caller's string data into the function's
/// pre-allocated data region space before the CALL. For scalar parameters,
/// compiles the argument expression normally. The CALL opcode pops scalar
/// arguments (and dummy values for STRING params) from the stack, stores them
/// into the function's variable slots, executes the function, and pushes
/// the return value onto the stack.
pub(crate) fn compile_user_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    func_info: &UserFunctionInfo,
) -> Result<(), Diagnostic> {
    // The analyzer wrote the call in normal form (`call-binding.md`): one
    // named input for each parameter, in declaration order, every one
    // supplied (P4018). A positional argument is one it did not bind.
    if let Some(ParamAssignmentKind::PositionalInput(input)) = func
        .param_assignment
        .iter()
        .find(|arg| matches!(arg, ParamAssignmentKind::PositionalInput(_)))
    {
        return Err(Diagnostic::internal_error_at(Label::span(
            input.expr.span(),
            "Positional argument of a function call the analyzer did not bind",
        )));
    }
    let args = collect_input_args(func);

    // Compile each argument with the corresponding parameter's OpType.
    // STRING parameters are copied into the function's data region before CALL;
    // a dummy zero is pushed for the stack pop count.
    for (i, arg) in args.iter().enumerate() {
        // Analysis rejects a call with more arguments than parameters.
        let passing = func_info.params.get(i).cloned().ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                func.name.span(),
                "Call has more arguments than the function has parameters",
            ))
        })?;
        match passing {
            ParamPassing::String(str_info) => {
                compile_string_arg(emitter, ctx, arg, &str_info, &func.name.span())?;
            }
            // The analyzer converted an argument of another width to the
            // parameter's type (ADR-0056).
            ParamPassing::Value => {
                compile_expr(emitter, ctx, arg)?;
            }
            ParamPassing::Reference => compile_reference_arg(emitter, ctx, arg)?,
        }
    }

    // If the function returns STRING, initialize the return string's header
    // in the data region before CALL so the function body can write to it.
    if let Some(ref ret_str) = func_info.return_string_info {
        emitter.emit_str_init(ret_str.data_offset, ret_str.max_length, ret_str.char_width);
    }

    let stack_depth = ctx.function_stack_depth(func_info.function_id, &func.name.span())?;
    emitter.emit_call(
        func_info.function_id,
        func_info.num_params,
        func_info.var_offset,
        stack_depth,
    );
    ctx.record_call_edge(func_info.function_id);
    // A function leaves its outputs in its own output variables; storing
    // them leaves the return value on the stack.
    store_outputs(
        emitter,
        ctx,
        CallOutputs::Variables(func_info.outputs.clone()),
        &func.param_assignment,
    )?;
    // For STRING-returning functions, the CALL leaves a buf_idx on the stack
    // (from emit_str_load_var in the function epilogue). The caller's
    // assignment path will consume it via emit_str_store_var.
    Ok(())
}

/// Compiles an argument passed to the string parameter `param`: copies it
/// into the parameter's run of the data region, then pushes a dummy value
/// for the slot `CALL` or `METHOD_CALL` pops. `span` is the call's.
pub(crate) fn compile_string_arg(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arg: &Expr,
    param: &StringParamInfo,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    // The parameter is initialized at its declared encoding, and the copy
    // has to agree with it.
    copy_string_arg(emitter, ctx, param, |emitter, ctx| {
        resolve_string_arg(emitter, ctx, arg, span, param.char_width)
    })
}

/// Copies a string argument into the string parameter `param`, then pushes
/// a dummy value for the slot `CALL` or `METHOD_CALL` pops. `source` puts
/// the argument in a slot of the data region and returns the slot's offset;
/// it runs once the parameter's header is initialized, so the caller
/// decides how the argument is resolved.
pub(crate) fn copy_string_arg(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    param: &StringParamInfo,
    source: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<u32, Diagnostic>,
) -> Result<(), Diagnostic> {
    // Initialize the destination header, then copy the string data.
    emitter.emit_str_init(param.data_offset, param.max_length, param.char_width);
    let src_offset = source(emitter, ctx)?;
    emitter.emit_str_load_var(src_offset);
    emitter.emit_str_store_var(param.data_offset);

    let zero_idx = ctx.add_i32_constant(0);
    emitter.emit_load_const_i32(zero_idx);
    Ok(())
}

/// Compiles an argument passed to a `VAR_IN_OUT` parameter: pushes a
/// reference to the argument variable, as `REF(x)` does.
///
/// When the argument is itself a `VAR_IN_OUT` parameter of the function
/// being compiled, its slot already holds a reference to the caller's
/// variable, and that reference is passed on. The analyzer has checked the
/// argument is a variable of the parameter's type (P4058, P4059); only a
/// named elementary variable, which occupies one slot, is supported.
fn compile_reference_arg(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arg: &Expr,
) -> Result<(), Diagnostic> {
    let name = match &arg.kind {
        ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
            Some(&named.name)
        }
        _ => None,
    };
    let Some(name) = name.filter(|name| passes_by_reference(ctx, name)) else {
        return Err(Diagnostic::not_implemented(Label::span(
            arg.span(),
            "VAR_IN_OUT argument that is not a named variable of an elementary type",
        )));
    };
    emit_reference(emitter, ctx, name)
}

/// Whether this backend passes the variable `name` to a `VAR_IN_OUT`
/// parameter: a variable of an elementary type, which has type info and
/// lives in its own slot, unlike a string, array, structure or instance,
/// which lives in the data region. A `VAR_IN_OUT` parameter of the function
/// being compiled is one.
pub(crate) fn passes_by_reference(ctx: &CompileContext, name: &Id) -> bool {
    ctx.var_type_info(name).is_some()
        && !ctx.string_var(name).is_some()
        && !ctx.array_var(name).is_some()
        && !ctx.struct_var(name).is_some()
        && !ctx.struct_array_var(name).is_some()
        && !ctx.fb_instance(name).is_some()
}

/// Pushes a reference to the variable `name`, which
/// [`passes_by_reference`]: its own, or the one a `VAR_IN_OUT` parameter's
/// slot holds, passed on.
pub(crate) fn emit_reference(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    name: &Id,
) -> Result<(), Diagnostic> {
    if let Some(ref_slot) = ctx.in_out_ref_slot(name) {
        emitter.emit_load_var_i64(ref_slot);
    } else {
        let reference = crate::compile_reference::reference_to(ctx, name)?;
        let pool_index = ctx.add_i64_constant(reference as i64);
        emitter.emit_load_const_i64(pool_index);
    }
    Ok(())
}
