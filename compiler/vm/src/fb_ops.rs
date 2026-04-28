//! Handler for the `FB_CALL` opcode.
//!
//! Extracted from `vm.rs::execute_with_hook` to keep the dispatch loop
//! tractable. Reads the FB type id from the operand stream, peeks the FB
//! instance reference from the operand stack, and dispatches to:
//!
//! - The intrinsic implementation for stdlib FBs (TON, TOF, TP, CTU, CTD,
//!   CTUD, SR, RS, R_TRIG, F_TRIG).
//! - Recursive `execute_with_hook` for user-defined FBs, with copy-in of
//!   fields → variable slots before the call and copy-out after.

use ironplc_container::{opcode, Container, FbTypeId, VarIndex};

use crate::debug_hook::DebugHook;
use crate::error::Trap;
#[cfg(feature = "profiling")]
use crate::profile::InstructionProfile;
use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use crate::vm::{execute_with_hook, read_u16_le, MAX_CALL_DEPTH};

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_fb_call<H: DebugHook>(
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
    let type_id = read_u16_le(bytecode, pc)?;
    let fb_ref = stack.peek()?.as_i32() as u32;
    let instance_start = fb_ref as usize;
    match type_id {
        opcode::fb_type::TON | opcode::fb_type::TOF | opcode::fb_type::TP => {
            let instance_size = crate::intrinsic::TIMER_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            let time = current_time_us as i64;
            match type_id {
                opcode::fb_type::TON => crate::intrinsic::ton(slice, time)?,
                opcode::fb_type::TOF => crate::intrinsic::tof(slice, time)?,
                opcode::fb_type::TP => crate::intrinsic::tp(slice, time)?,
                _ => unreachable!(),
            }
        }
        opcode::fb_type::CTU => {
            let instance_size = crate::intrinsic::CTU_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::ctu(slice)?;
        }
        opcode::fb_type::CTD => {
            let instance_size = crate::intrinsic::CTD_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::ctd(slice)?;
        }
        opcode::fb_type::CTUD => {
            let instance_size = crate::intrinsic::CTUD_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::ctud(slice)?;
        }
        opcode::fb_type::SR => {
            let instance_size = crate::intrinsic::SR_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::sr(slice)?;
        }
        opcode::fb_type::RS => {
            let instance_size = crate::intrinsic::RS_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::rs(slice)?;
        }
        opcode::fb_type::R_TRIG => {
            let instance_size = crate::intrinsic::R_TRIG_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::r_trig(slice)?;
        }
        opcode::fb_type::F_TRIG => {
            let instance_size = crate::intrinsic::F_TRIG_INSTANCE_FIELDS * 8;
            let instance_end = instance_start + instance_size;
            if instance_end > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
            }
            let slice = &mut data_region[instance_start..instance_end];
            crate::intrinsic::f_trig(slice)?;
        }
        _ => {
            // User-defined FB: look up in the container's user FB table.
            let fb_type_id = FbTypeId::new(type_id);
            let user_fb = container
                .type_section
                .as_ref()
                .and_then(|ts| ts.user_fb_types.iter().find(|d| d.type_id == fb_type_id))
                .ok_or(Trap::InvalidFbTypeId(fb_type_id))?;

            let func_id = user_fb.function_id;
            let var_off = user_fb.var_offset;
            let num_fields = user_fb.num_fields as usize;

            let func = container
                .code
                .get_function(func_id)
                .ok_or(Trap::InvalidFunctionId(func_id))?;
            let func_bytecode = container
                .code
                .get_function_bytecode(func_id)
                .ok_or(Trap::InvalidFunctionId(func_id))?;

            // Copy-in: data region fields -> variable table slots.
            for i in 0..num_fields {
                let offset = instance_start + i * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[offset..offset + 8]);
                variables.store(
                    VarIndex::new(var_off + i as u16),
                    Slot::from_i64(i64::from_le_bytes(buf)),
                )?;
            }

            // Execute the FB body.
            let func_scope = VariableScope {
                shared_globals_size: scope.shared_globals_size,
                instance_offset: var_off,
                instance_count: func.num_locals,
            };
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

            // Copy-out: variable table slots -> data region fields.
            for i in 0..num_fields {
                let offset = instance_start + i * 8;
                let val = variables.load(VarIndex::new(var_off + i as u16))?;
                data_region[offset..offset + 8]
                    .copy_from_slice(&val.as_i64().to_le_bytes());
            }
        }
    }
    Ok(())
}
