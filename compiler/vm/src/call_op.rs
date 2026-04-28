//! Handler for the `CALL` opcode.
//!
//! Extracted from `vm.rs::execute_with_hook` to keep the dispatch loop
//! tractable. Reads the function ID and var-table offset from the operand
//! stream, pops arguments into the callee's parameter slots, then
//! recursively invokes `execute_with_hook` on the callee's bytecode.

use ironplc_container::{Container, FunctionId, VarIndex};

use crate::debug_hook::DebugHook;
use crate::error::Trap;
#[cfg(feature = "profiling")]
use crate::profile::InstructionProfile;
use crate::stack::OperandStack;
use crate::variable_table::{VariableScope, VariableTable};
use crate::dispatch::{execute_with_hook, read_u16_le};
use crate::vm::MAX_CALL_DEPTH;

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_call<H: DebugHook>(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    scope: &VariableScope,
    current_time_us: u64,
    depth: u32,
    #[cfg(feature = "profiling")] profile: &mut InstructionProfile,
    hook: &mut H,
) -> Result<(), Trap> {
    let func_id = read_u16_le(bytecode, pc)?;
    let var_offset = read_u16_le(bytecode, pc)?;
    let func = container
        .code
        .get_function(FunctionId::new(func_id))
        .ok_or(Trap::InvalidFunctionId(FunctionId::new(func_id)))?;
    let func_bytecode = container
        .code
        .get_function_bytecode(FunctionId::new(func_id))
        .ok_or(Trap::InvalidFunctionId(FunctionId::new(func_id)))?;

    let func_scope = VariableScope {
        shared_globals_size: scope.shared_globals_size,
        instance_offset: var_offset,
        instance_count: func.num_locals,
    };

    // Pop arguments from the stack into the callee's parameter slots
    // (reverse order, since the last arg is on top of stack).
    for i in (0..func.num_params).rev() {
        let val = stack.pop()?;
        variables.store(VarIndex::new(var_offset + i), val)?;
    }

    // Recursively execute the callee. A future iterative-dispatch
    // rewrite could replace this recursion with an explicit return-
    // address stack; until then the depth counter bounds it.
    if depth >= MAX_CALL_DEPTH {
        return Err(Trap::CallStackOverflow);
    }
    execute_with_hook(
        func_bytecode,
        container,
        stack,
        variables,
        data_region,
        temp_buf,
        max_temp_buf_bytes,
        &func_scope,
        current_time_us,
        depth + 1,
        #[cfg(feature = "profiling")]
        profile,
        hook,
    )?;
    Ok(())
}
