//! Bytecode dispatch loop for the IronPLC VM.
//!
//! Hosts `execute_with_hook` (the main dispatch loop) and the small set
//! of helpers it needs: opcode-handler macros, bytecode-stream readers,
//! and `StackFmtBuf` (a stack-allocated number→string formatter used by
//! BUILTIN's CONV_*_TO_STR cases).
//!
//! Big match-arm bodies were extracted into per-family modules
//! (`builtin`, `string_ops`, `fb_ops`, `array_ops`, `call_op`) so this
//! file stays focused on the dispatch shape itself.

use ironplc_container::{opcode, ConstantIndex, Container, VarIndex};

use crate::builtin;
use crate::debug_hook::{DebugHook, NoopDebugHook};
use crate::error::Trap;
#[cfg(feature = "profiling")]
use crate::profile::InstructionProfile;
use crate::stack::OperandStack;
use crate::string_ops;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};

/// Binary operation: pop b then a, compute result, push.
macro_rules! binop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $a:ident, $b:ident, $result:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Comparison: pop b then a, compare, push i32 boolean.
macro_rules! cmpop {
    ($stack:expr, $as_ty:ident, $a:ident, $b:ident, $cond:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::from_i32(if $cond { 1 } else { 0 }))?;
    }};
}

/// Unary operation: pop one, compute, push.
macro_rules! unaryop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $a:ident, $result:expr) => {{
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Checked division: pop b then a, check b != zero, compute, push.
macro_rules! checked_divop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $zero:expr, $a:ident, $b:ident, $result:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        if $b == $zero {
            return Err(Trap::DivideByZero);
        }
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Load constant from pool: read index, look up, push.
macro_rules! load_const {
    ($bytecode:expr, $pc:expr, $container:expr, $stack:expr, $get:ident, $from:ident) => {{
        let index = read_u16_le($bytecode, &mut $pc)?;
        let value = $container
            .constant_pool
            .$get(ConstantIndex::new(index))
            .map_err(|_| Trap::InvalidConstantIndex(ConstantIndex::new(index)))?;
        $stack.push(Slot::$from(value))?;
    }};
}

/// Executes bytecode until RET_VOID or a trap, using a no-op debug hook.
///
/// This is a thin wrapper around [`execute_with_hook`] that supplies a
/// [`NoopDebugHook`]. Existing call sites use this entry point so that the
/// debug-hook plumbing imposes no overhead on VMs that do not need
/// instruction-level callbacks (the noop hook is a ZST and inlines away).
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute(
    bytecode: &[u8],
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
) -> Result<(), Trap> {
    let mut hook = NoopDebugHook;
    execute_with_hook(
        bytecode,
        container,
        stack,
        variables,
        data_region,
        temp_buf,
        max_temp_buf_bytes,
        scope,
        current_time_us,
        depth,
        #[cfg(feature = "profiling")]
        profile,
        &mut hook,
    )
}

/// Executes bytecode until RET_VOID or a trap, invoking `hook.before_instruction`
/// before each opcode.
///
/// This is a free function so that the borrow checker can see
/// independent borrows of container (immutable) vs stack/variables
/// (mutable). It is generic over the hook type so that the noop hook
/// monomorphization compiles to identical code as before; only callers
/// that supply a real hook pay any runtime cost.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_with_hook<H: DebugHook>(
    bytecode: &[u8],
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
    let mut pc: usize = 0;
    let mut temp_alloc = string_ops::TempBufAllocator::new(max_temp_buf_bytes);

    while pc < bytecode.len() {
        let op = bytecode[pc];
        // Notify the debug hook before advancing pc so the hook sees the
        // offset of the opcode itself, not its operand bytes. With
        // NoopDebugHook this call is inlined away to nothing.
        hook.before_instruction(pc, op);
        pc += 1;

        #[cfg(feature = "profiling")]
        profile.record(op);

        match op {
            // --- Load constants ---
            opcode::LOAD_CONST_I32 => {
                load_const!(bytecode, pc, container, stack, get_i32, from_i32)
            }
            opcode::LOAD_CONST_I64 => {
                load_const!(bytecode, pc, container, stack, get_i64, from_i64)
            }
            opcode::LOAD_CONST_F32 => {
                load_const!(bytecode, pc, container, stack, get_f32, from_f32)
            }
            opcode::LOAD_CONST_F64 => {
                load_const!(bytecode, pc, container, stack, get_f64, from_f64)
            }
            opcode::LOAD_TRUE => {
                stack.push(Slot::from_i32(1))?;
            }
            opcode::LOAD_FALSE => {
                stack.push(Slot::from_i32(0))?;
            }
            // --- Load/store variables (type-erased slots) ---
            opcode::LOAD_VAR_I32
            | opcode::LOAD_VAR_I64
            | opcode::LOAD_VAR_F32
            | opcode::LOAD_VAR_F64 => {
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(index)?;
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32
            | opcode::STORE_VAR_I64
            | opcode::STORE_VAR_F32
            | opcode::STORE_VAR_F64 => {
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(index)?;
                let slot = stack.pop()?;
                variables.store(index, slot)?;
            }
            // --- Indirect load/store (reference dereference) ---
            opcode::LOAD_INDIRECT => {
                let ref_slot = stack.pop()?;
                if ref_slot.is_null_ref() {
                    return Err(Trap::NullDereference);
                }
                let target_index = ref_slot
                    .as_var_index()
                    .ok_or(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))?;
                scope.check_access(target_index)?;
                let value = variables.load(target_index)?;
                stack.push(value)?;
            }
            opcode::STORE_INDIRECT => {
                let ref_slot = stack.pop()?;
                if ref_slot.is_null_ref() {
                    return Err(Trap::NullDereference);
                }
                let target_index = ref_slot
                    .as_var_index()
                    .ok_or(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))?;
                scope.check_access(target_index)?;
                let value = stack.pop()?;
                variables.store(target_index, value)?;
            }
            // --- Integer arithmetic (wrapping) ---
            opcode::ADD_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_add(b)),
            opcode::SUB_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_sub(b)),
            opcode::MUL_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_mul(b)),
            opcode::ADD_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_add(b)),
            opcode::SUB_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_sub(b)),
            opcode::MUL_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_mul(b)),
            // --- Integer division (checked for zero) ---
            opcode::DIV_I32 => {
                checked_divop!(stack, as_i32, from_i32, 0i32, a, b, a.wrapping_div(b))
            }
            opcode::MOD_I32 => {
                checked_divop!(stack, as_i32, from_i32, 0i32, a, b, a.wrapping_rem(b))
            }
            opcode::DIV_I64 => {
                checked_divop!(stack, as_i64, from_i64, 0i64, a, b, a.wrapping_div(b))
            }
            opcode::MOD_I64 => {
                checked_divop!(stack, as_i64, from_i64, 0i64, a, b, a.wrapping_rem(b))
            }
            // --- Unsigned integer division (checked for zero) ---
            opcode::DIV_U32 => checked_divop!(
                stack,
                as_i32,
                from_i32,
                0i32,
                a,
                b,
                ((a as u32) / (b as u32)) as i32
            ),
            opcode::MOD_U32 => checked_divop!(
                stack,
                as_i32,
                from_i32,
                0i32,
                a,
                b,
                ((a as u32) % (b as u32)) as i32
            ),
            opcode::DIV_U64 => checked_divop!(
                stack,
                as_i64,
                from_i64,
                0i64,
                a,
                b,
                ((a as u64) / (b as u64)) as i64
            ),
            opcode::MOD_U64 => checked_divop!(
                stack,
                as_i64,
                from_i64,
                0i64,
                a,
                b,
                ((a as u64) % (b as u64)) as i64
            ),
            // --- Float arithmetic ---
            opcode::ADD_F32 => binop!(stack, as_f32, from_f32, a, b, a + b),
            opcode::SUB_F32 => binop!(stack, as_f32, from_f32, a, b, a - b),
            opcode::MUL_F32 => binop!(stack, as_f32, from_f32, a, b, a * b),
            opcode::DIV_F32 => binop!(stack, as_f32, from_f32, a, b, a / b),
            opcode::ADD_F64 => binop!(stack, as_f64, from_f64, a, b, a + b),
            opcode::SUB_F64 => binop!(stack, as_f64, from_f64, a, b, a - b),
            opcode::MUL_F64 => binop!(stack, as_f64, from_f64, a, b, a * b),
            opcode::DIV_F64 => binop!(stack, as_f64, from_f64, a, b, a / b),
            // --- Negation ---
            opcode::NEG_I32 => unaryop!(stack, as_i32, from_i32, a, a.wrapping_neg()),
            opcode::NEG_I64 => unaryop!(stack, as_i64, from_i64, a, a.wrapping_neg()),
            opcode::NEG_F32 => unaryop!(stack, as_f32, from_f32, a, -a),
            opcode::NEG_F64 => unaryop!(stack, as_f64, from_f64, a, -a),
            // --- Truncation ---
            opcode::TRUNC_I8 => unaryop!(stack, as_i32, from_i32, a, (a as i8) as i32),
            opcode::TRUNC_U8 => unaryop!(stack, as_i32, from_i32, a, (a as u8) as i32),
            opcode::TRUNC_I16 => unaryop!(stack, as_i32, from_i32, a, (a as i16) as i32),
            opcode::TRUNC_U16 => unaryop!(stack, as_i32, from_i32, a, (a as u16) as i32),
            // --- Signed comparison ---
            opcode::EQ_I32 => cmpop!(stack, as_i32, a, b, a == b),
            opcode::NE_I32 => cmpop!(stack, as_i32, a, b, a != b),
            opcode::LT_I32 => cmpop!(stack, as_i32, a, b, a < b),
            opcode::LE_I32 => cmpop!(stack, as_i32, a, b, a <= b),
            opcode::GT_I32 => cmpop!(stack, as_i32, a, b, a > b),
            opcode::GE_I32 => cmpop!(stack, as_i32, a, b, a >= b),
            opcode::EQ_I64 => cmpop!(stack, as_i64, a, b, a == b),
            opcode::NE_I64 => cmpop!(stack, as_i64, a, b, a != b),
            opcode::LT_I64 => cmpop!(stack, as_i64, a, b, a < b),
            opcode::LE_I64 => cmpop!(stack, as_i64, a, b, a <= b),
            opcode::GT_I64 => cmpop!(stack, as_i64, a, b, a > b),
            opcode::GE_I64 => cmpop!(stack, as_i64, a, b, a >= b),
            // --- Unsigned comparison ---
            opcode::LT_U32 => cmpop!(stack, as_i32, a, b, (a as u32) < (b as u32)),
            opcode::LE_U32 => cmpop!(stack, as_i32, a, b, (a as u32) <= (b as u32)),
            opcode::GT_U32 => cmpop!(stack, as_i32, a, b, (a as u32) > (b as u32)),
            opcode::GE_U32 => cmpop!(stack, as_i32, a, b, (a as u32) >= (b as u32)),
            opcode::LT_U64 => cmpop!(stack, as_i64, a, b, (a as u64) < (b as u64)),
            opcode::LE_U64 => cmpop!(stack, as_i64, a, b, (a as u64) <= (b as u64)),
            opcode::GT_U64 => cmpop!(stack, as_i64, a, b, (a as u64) > (b as u64)),
            opcode::GE_U64 => cmpop!(stack, as_i64, a, b, (a as u64) >= (b as u64)),
            // --- Float comparison ---
            opcode::EQ_F32 => cmpop!(stack, as_f32, a, b, a == b),
            opcode::NE_F32 => cmpop!(stack, as_f32, a, b, a != b),
            opcode::LT_F32 => cmpop!(stack, as_f32, a, b, a < b),
            opcode::LE_F32 => cmpop!(stack, as_f32, a, b, a <= b),
            opcode::GT_F32 => cmpop!(stack, as_f32, a, b, a > b),
            opcode::GE_F32 => cmpop!(stack, as_f32, a, b, a >= b),
            opcode::EQ_F64 => cmpop!(stack, as_f64, a, b, a == b),
            opcode::NE_F64 => cmpop!(stack, as_f64, a, b, a != b),
            opcode::LT_F64 => cmpop!(stack, as_f64, a, b, a < b),
            opcode::LE_F64 => cmpop!(stack, as_f64, a, b, a <= b),
            opcode::GT_F64 => cmpop!(stack, as_f64, a, b, a > b),
            opcode::GE_F64 => cmpop!(stack, as_f64, a, b, a >= b),
            // --- Boolean logic ---
            opcode::BOOL_AND => cmpop!(stack, as_i32, a, b, (a != 0) && (b != 0)),
            opcode::BOOL_OR => cmpop!(stack, as_i32, a, b, (a != 0) || (b != 0)),
            opcode::BOOL_XOR => cmpop!(stack, as_i32, a, b, (a != 0) != (b != 0)),
            opcode::BOOL_NOT => unaryop!(stack, as_i32, from_i32, a, if a == 0 { 1 } else { 0 }),
            // --- Bitwise (32-bit) ---
            opcode::BIT_AND_32 => binop!(stack, as_i32, from_i32, a, b, a & b),
            opcode::BIT_OR_32 => binop!(stack, as_i32, from_i32, a, b, a | b),
            opcode::BIT_XOR_32 => binop!(stack, as_i32, from_i32, a, b, a ^ b),
            opcode::BIT_NOT_32 => unaryop!(stack, as_i32, from_i32, a, !a),
            // --- Bitwise (64-bit) ---
            opcode::BIT_AND_64 => binop!(stack, as_i64, from_i64, a, b, a & b),
            opcode::BIT_OR_64 => binop!(stack, as_i64, from_i64, a, b, a | b),
            opcode::BIT_XOR_64 => binop!(stack, as_i64, from_i64, a, b, a ^ b),
            opcode::BIT_NOT_64 => unaryop!(stack, as_i64, from_i64, a, !a),
            // --- Control flow ---
            opcode::JMP => {
                let offset = read_i16_le(bytecode, &mut pc)?;
                pc = (pc as isize + offset as isize) as usize;
            }
            opcode::JMP_IF_NOT => {
                let offset = read_i16_le(bytecode, &mut pc)?;
                let cond = stack.pop()?.as_i32();
                if cond == 0 {
                    pc = (pc as isize + offset as isize) as usize;
                }
            }
            opcode::BUILTIN => builtin::handle_builtin(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                &mut temp_alloc,
            )?,
            opcode::CALL => crate::call_op::handle_call(
                bytecode,
                &mut pc,
                container,
                stack,
                variables,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                scope,
                current_time_us,
                depth,
                #[cfg(feature = "profiling")]
                profile,
                hook,
            )?,
            opcode::RET => {
                // Return value is already on the stack; just return from execute().
                return Ok(());
            }
            // --- String opcodes ---
            //
            // Strings are variable-length and can't fit in the fixed-width
            // 64-bit stack slots. They live in two places:
            //
            //   1. **Data region**: persistent storage for STRING variables.
            //      Each string is laid out per ADR-0015 as:
            //        [max_length: u16][cur_length: u16][data: up to max_length bytes]
            //      The `data_offset` (byte offset into the data region) identifies
            //      each string and is baked into the bytecode operand.
            //
            //   2. **Temp buffers**: short-lived staging area for intermediate
            //      string values. Same [max][cur][data] layout. The temp buffer
            //      pool is a flat byte array divided into equal-sized slots; a
            //      `buf_idx` (which fits in one stack slot) identifies which temp
            //      buffer holds the data. A bump allocator (`TempBufAllocator`)
            //      hands out temp buffers within a single function call.
            //
            // The typical pattern for string assignment is:
            //   LOAD_CONST_STR pool[i]    -- copy literal → temp buf, push buf_idx
            //   STR_STORE_VAR  offset     -- pop buf_idx, copy temp buf → data region

            // STR_INIT: Initialize a string variable's header in the data region.
            //
            // Operands: data_offset (u16), max_length (u16)
            // Stack effect: none
            //
            // Sets max_length and zeros cur_length. This is emitted once per
            // STRING variable during program initialization, before any values
            // are stored. STR_STORE_VAR relies on max_length being set here
            // to enforce the capacity bound.
            opcode::STR_INIT => string_ops::handle_str_init(bytecode, &mut pc, data_region)?,
            opcode::LOAD_CONST_STR => string_ops::handle_load_const_str(
                bytecode,
                &mut pc,
                container,
                stack,
                temp_buf,
                max_temp_buf_bytes,
                &mut temp_alloc,
            )?,
            opcode::STR_STORE_VAR => string_ops::handle_str_store_var(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
            )?,
            opcode::STR_LOAD_VAR => string_ops::handle_str_load_var(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                &mut temp_alloc,
            )?,
            opcode::LEN_STR => {
                string_ops::handle_len_str(bytecode, &mut pc, stack, data_region)?
            }
            opcode::FIND_STR => {
                string_ops::handle_find_str(bytecode, &mut pc, stack, data_region)?
            }
            opcode::REPLACE_STR => string_ops::handle_replace_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::INSERT_STR => string_ops::handle_insert_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::DELETE_STR => string_ops::handle_delete_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::LEFT_STR => string_ops::handle_left_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::RIGHT_STR => string_ops::handle_right_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::MID_STR => string_ops::handle_mid_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::CONCAT_STR => string_ops::handle_concat_str(
                bytecode,
                &mut pc,
                stack,
                data_region,
                temp_buf,
                &mut temp_alloc,
            )?,
            opcode::STR_INIT_ARRAY => string_ops::handle_str_init_array(
                bytecode,
                &mut pc,
                container,
                variables,
                data_region,
                scope,
            )?,
            opcode::STR_LOAD_ARRAY_ELEM => string_ops::handle_str_load_array_elem(
                bytecode,
                &mut pc,
                container,
                stack,
                variables,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                &mut temp_alloc,
                scope,
            )?,
            opcode::STR_STORE_ARRAY_ELEM => string_ops::handle_str_store_array_elem(
                bytecode,
                &mut pc,
                container,
                stack,
                variables,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                scope,
            )?,

            opcode::POP => {
                stack.pop()?;
            }
            opcode::DUP => {
                stack.dup()?;
            }
            opcode::SWAP => {
                stack.swap()?;
            }
            // --- Function block opcodes ---
            opcode::FB_LOAD_INSTANCE => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(var_index)?;
                let slot = variables.load(var_index)?;
                stack.push(slot)?;
            }
            opcode::FB_STORE_PARAM => {
                let field = read_u8(bytecode, &mut pc)? as u16;
                let value = stack.pop()?;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                data_region[offset..offset + 8].copy_from_slice(&value.as_i64().to_le_bytes());
            }
            opcode::FB_LOAD_PARAM => {
                let field = read_u8(bytecode, &mut pc)? as u16;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[offset..offset + 8]);
                stack.push(Slot::from_i64(i64::from_le_bytes(buf)))?;
            }
            opcode::FB_CALL => crate::fb_ops::handle_fb_call(
                bytecode,
                &mut pc,
                container,
                stack,
                variables,
                data_region,
                temp_buf,
                max_temp_buf_bytes,
                scope,
                current_time_us,
                depth,
                #[cfg(feature = "profiling")]
                profile,
                hook,
            )?,
            // --- Array opcodes ---
            opcode::LOAD_ARRAY => crate::array_ops::handle_load_array(
                bytecode, &mut pc, container, stack, variables, data_region, scope,
            )?,
            opcode::STORE_ARRAY => crate::array_ops::handle_store_array(
                bytecode, &mut pc, container, stack, variables, data_region, scope,
            )?,
            opcode::LOAD_ARRAY_DEREF => crate::array_ops::handle_load_array_deref(
                bytecode, &mut pc, container, stack, variables, data_region, scope,
            )?,
            opcode::STORE_ARRAY_DEREF => crate::array_ops::handle_store_array_deref(
                bytecode, &mut pc, container, stack, variables, data_region, scope,
            )?,
            opcode::RET_VOID => {
                return Ok(());
            }
            _ => {
                return Err(Trap::InvalidInstruction(op));
            }
        }
    }

    Ok(())
}


/// Reads a single byte from bytecode at pc, advancing pc by 1.
pub(crate) fn read_u8(bytecode: &[u8], pc: &mut usize) -> Result<u8, Trap> {
    if *pc >= bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = bytecode[*pc];
    *pc += 1;
    Ok(value)
}

/// Reads a little-endian u16 from bytecode at pc, advancing pc by 2.
pub(crate) fn read_u16_le(bytecode: &[u8], pc: &mut usize) -> Result<u16, Trap> {
    let end = *pc + 2;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = u16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc = end;
    Ok(value)
}

/// Reads a little-endian u32 from bytecode at pc, advancing pc by 4.
pub(crate) fn read_u32_le(bytecode: &[u8], pc: &mut usize) -> Result<u32, Trap> {
    let end = *pc + 4;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = u32::from_le_bytes([
        bytecode[*pc],
        bytecode[*pc + 1],
        bytecode[*pc + 2],
        bytecode[*pc + 3],
    ]);
    *pc = end;
    Ok(value)
}

/// Reads a little-endian i16 from bytecode at pc, advancing pc by 2.
pub(crate) fn read_i16_le(bytecode: &[u8], pc: &mut usize) -> Result<i16, Trap> {
    let end = *pc + 2;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = i16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc = end;
    Ok(value)
}

/// A small stack-allocated buffer for formatting numbers as strings.
///
/// Used by CONV_I32_TO_STR, CONV_U32_TO_STR, and CONV_F32_TO_STR to
/// avoid heap allocation. 48 bytes is enough for any i32, u32, or f32
/// decimal representation.
pub(crate) struct StackFmtBuf {
    buf: [u8; 48],
    len: usize,
}

impl StackFmtBuf {
    pub(crate) fn new() -> Self {
        Self {
            buf: [0u8; 48],
            len: 0,
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl core::fmt::Write for StackFmtBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.len;
        let to_copy = bytes.len().min(remaining);
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        Ok(())
    }
}

// These inline tests build bytecode from raw hex literals via the
// `single_function_container` and `steel_thread_container` helpers.
// They are gated behind the same `legacy_bytecode_tests` feature as the
// integration tests under `vm/tests/` for the duration of the encoding
// renumbering work — see specs/plans/2026-04-28-opcode-encoding-reorganization.md.
// Phase 5 conformance will convert the hex literals to named constants
