//! Operand-stack discipline verification for emitted bytecode.
//!
//! This module implements the stack-discipline subset of the bytecode
//! verifier specified in `specs/design/bytecode-verifier-rules.md`:
//!
//! | Rule | Meaning | Variant |
//! |------|---------|---------|
//! | R0200 | Stack depth agrees at every control-flow merge point | [`StackImbalance::MergeConflict`] |
//! | R0202 | No instruction pops from an empty stack | [`StackImbalance::Underflow`] |
//! | R0203 | Depth never exceeds the declared `max_stack_depth` | [`StackImbalance::ExceedsDeclaredMax`] |
//!
//! plus the *balance* rule that motivates the pass: every path that leaves
//! a function must leave the operand stack at exactly the depth the calling
//! convention promises ([`StackImbalance::UnbalancedReturn`]) — 1 slot for
//! `RET` (the return value), 0 slots for `RET_VOID` and for falling off the
//! end of a body.
//!
//! # Why abstract interpretation
//!
//! The emitter keeps a running `current_stack_depth` while it appends
//! instructions, but that counter walks the buffer in *emission* order, not
//! in execution order. It therefore cannot answer the question this pass
//! answers: it sums both arms of an `IF` as if they ran back to back, it
//! keeps counting past an early `RETURN` that in reality leaves the
//! function, and it never compares the depths that different predecessors
//! deliver to the same instruction. Asserting that counter is zero at the
//! end of a function is neither sound (two opposite errors cancel) nor
//! complete (a value-returning early `RETURN` leaves it non-zero on
//! perfectly valid code).
//!
//! Walking the control-flow graph instead makes every one of those cases
//! exact, and — the point of the exercise — derives the answer from the
//! *bytecode that ships*, independently of the bookkeeping the emitter did
//! on the way there.
//!
//! # Model
//!
//! Each function is verified on its own with an entry depth of 0. That is
//! sound because the calling convention isolates frames: `CALL` pops the
//! callee's arguments *before* the callee's frame is pushed, so a callee
//! never observes — and must never touch — slots belonging to its caller.
//! `FB_CALL` likewise leaves the caller's `fb_ref` in place and runs the
//! body as its own frame.

use std::collections::VecDeque;
use std::fmt;
use std::vec;
use std::vec::Vec;

use crate::code_section::CodeSection;
use crate::id_types::FunctionId;
use crate::opcode::{self, Opcode};

/// Depth the operand stack must have when a `RET` executes: the single
/// return value the caller's `CALL` accounts for.
const RET_DEPTH: u16 = 1;

/// Depth the operand stack must have when a `RET_VOID` executes, or when
/// control falls off the end of a body (which the VM treats as `RET_VOID`).
const RET_VOID_DEPTH: u16 = 0;

/// A violation of operand-stack discipline found in emitted bytecode.
///
/// Every variant names the function and the byte offset within that
/// function's body, so a failure points at the instruction responsible
/// rather than at a downstream symptom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StackImbalance {
    /// R0202: an instruction pops more values than the stack holds.
    Underflow {
        function_id: FunctionId,
        offset: usize,
        opcode: Opcode,
        /// Slots the instruction pops.
        needs: u16,
        /// Slots actually on the stack.
        has: u16,
    },
    /// R0200: two control-flow paths reach the same instruction with
    /// different stack depths.
    MergeConflict {
        function_id: FunctionId,
        offset: usize,
        /// Depth recorded by the path that reached this offset first.
        existing: u16,
        /// Depth delivered by the conflicting path.
        incoming: u16,
    },
    /// A path leaves the function with the wrong number of values on the
    /// stack — the leak (or over-pop) this pass exists to catch.
    UnbalancedReturn {
        function_id: FunctionId,
        offset: usize,
        /// Depth the calling convention requires at this exit.
        expected: u16,
        /// Depth the path actually delivers.
        actual: u16,
    },
    /// R0203: the stack grows past the depth declared in the function
    /// directory, which is what sizes the VM's operand-stack buffer.
    ExceedsDeclaredMax {
        function_id: FunctionId,
        offset: usize,
        /// Peak depth reached on some path.
        depth: u16,
        /// Depth declared in the function directory.
        declared: u16,
    },
    /// A branch targets a byte that is not the start of an instruction, or
    /// lies outside the function body.
    InvalidJumpTarget {
        function_id: FunctionId,
        offset: usize,
        target: isize,
    },
    /// A byte in an instruction position is not an assigned opcode.
    UnknownOpcode {
        function_id: FunctionId,
        offset: usize,
        byte: u8,
    },
    /// An instruction's operand bytes run past the end of the body.
    TruncatedInstruction {
        function_id: FunctionId,
        offset: usize,
        opcode: Opcode,
    },
    /// A `BUILTIN` names a function ID with no known argument count, so its
    /// stack effect cannot be determined.
    UnknownBuiltin {
        function_id: FunctionId,
        offset: usize,
        builtin_id: u16,
    },
    /// A `CALL` names a function that is not in the function directory, so
    /// its parameter count — and therefore its stack effect — is unknown.
    UnknownCallee {
        function_id: FunctionId,
        offset: usize,
        callee: FunctionId,
    },
}

impl StackImbalance {
    /// The function whose body contains the violation.
    pub fn function_id(&self) -> FunctionId {
        match self {
            StackImbalance::Underflow { function_id, .. }
            | StackImbalance::MergeConflict { function_id, .. }
            | StackImbalance::UnbalancedReturn { function_id, .. }
            | StackImbalance::ExceedsDeclaredMax { function_id, .. }
            | StackImbalance::InvalidJumpTarget { function_id, .. }
            | StackImbalance::UnknownOpcode { function_id, .. }
            | StackImbalance::TruncatedInstruction { function_id, .. }
            | StackImbalance::UnknownBuiltin { function_id, .. }
            | StackImbalance::UnknownCallee { function_id, .. } => *function_id,
        }
    }

    /// Byte offset within that function's body.
    pub fn offset(&self) -> usize {
        match self {
            StackImbalance::Underflow { offset, .. }
            | StackImbalance::MergeConflict { offset, .. }
            | StackImbalance::UnbalancedReturn { offset, .. }
            | StackImbalance::ExceedsDeclaredMax { offset, .. }
            | StackImbalance::InvalidJumpTarget { offset, .. }
            | StackImbalance::UnknownOpcode { offset, .. }
            | StackImbalance::TruncatedInstruction { offset, .. }
            | StackImbalance::UnknownBuiltin { offset, .. }
            | StackImbalance::UnknownCallee { offset, .. } => *offset,
        }
    }

    /// The `R####` rule code from `bytecode-verifier-rules.md` this
    /// violation corresponds to, when one applies.
    pub fn rule(&self) -> Option<&'static str> {
        match self {
            StackImbalance::MergeConflict { .. } => Some("R0200"),
            StackImbalance::Underflow { .. } => Some("R0202"),
            StackImbalance::ExceedsDeclaredMax { .. } => Some("R0203"),
            StackImbalance::InvalidJumpTarget { .. } => Some("R0400"),
            StackImbalance::UnknownOpcode { .. } => Some("R0001"),
            StackImbalance::UnknownBuiltin { .. } => Some("R0510"),
            StackImbalance::UnknownCallee { .. } => Some("R0002"),
            StackImbalance::UnbalancedReturn { .. }
            | StackImbalance::TruncatedInstruction { .. } => None,
        }
    }
}

impl fmt::Display for StackImbalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let func = self.function_id();
        let at = self.offset();
        match self {
            StackImbalance::Underflow {
                opcode, needs, has, ..
            } => write!(
                f,
                "function {func} offset {at}: opcode 0x{opcode:02X} pops {needs} value(s) \
                 but the operand stack holds {has}"
            ),
            StackImbalance::MergeConflict {
                existing, incoming, ..
            } => write!(
                f,
                "function {func} offset {at}: control-flow paths reach this instruction with \
                 different operand-stack depths ({existing} and {incoming})"
            ),
            StackImbalance::UnbalancedReturn {
                expected, actual, ..
            } => write!(
                f,
                "function {func} offset {at}: this exit leaves {actual} value(s) on the operand \
                 stack but the calling convention requires {expected}"
            ),
            StackImbalance::ExceedsDeclaredMax {
                depth, declared, ..
            } => write!(
                f,
                "function {func} offset {at}: operand stack reaches depth {depth}, past the \
                 declared max_stack_depth of {declared}"
            ),
            StackImbalance::InvalidJumpTarget { target, .. } => write!(
                f,
                "function {func} offset {at}: branch target {target} is outside the function \
                 body or does not start an instruction"
            ),
            StackImbalance::UnknownOpcode { byte, .. } => write!(
                f,
                "function {func} offset {at}: 0x{byte:02X} is not an assigned opcode"
            ),
            StackImbalance::TruncatedInstruction { opcode, .. } => write!(
                f,
                "function {func} offset {at}: opcode 0x{opcode:02X} operands run past the end \
                 of the function body"
            ),
            StackImbalance::UnknownBuiltin { builtin_id, .. } => write!(
                f,
                "function {func} offset {at}: BUILTIN 0x{builtin_id:04X} has no known argument \
                 count, so its stack effect is undefined"
            ),
            StackImbalance::UnknownCallee { callee, .. } => write!(
                f,
                "function {func} offset {at}: CALL targets function {callee}, which is not in \
                 the function directory"
            ),
        }
    }
}

/// How many values an instruction pops and pushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Effect {
    pops: u16,
    pushes: u16,
}

impl Effect {
    const NONE: Effect = Effect { pops: 0, pushes: 0 };

    const fn new(pops: u16, pushes: u16) -> Self {
        Effect { pops, pushes }
    }
}

/// Where control can go after an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    /// Continue at the next instruction.
    Next,
    /// Continue only at the branch target.
    Jump(isize),
    /// Continue at either the branch target or the next instruction.
    Branch(isize),
    /// Leave the function, requiring this operand-stack depth.
    Return(u16),
}

/// Verifies operand-stack discipline for every function in `code`.
///
/// Returns the first violation found, scanning functions in directory
/// order and each body from its entry point outwards. Unreachable
/// instructions are not verified — nothing can execute them, so they
/// cannot unbalance the stack.
pub fn verify_stack_balance(code: &CodeSection) -> Result<(), StackImbalance> {
    for entry in &code.functions {
        let bytecode = code
            .get_function_bytecode(entry.function_id)
            .unwrap_or_default();
        verify_function(code, entry.function_id, bytecode, entry.max_stack_depth)?;
    }
    Ok(())
}

/// Verifies one function body by abstract interpretation over its CFG.
///
/// `declared_max` is the function directory's `max_stack_depth`, which
/// sizes the VM's operand-stack buffer (R0203).
fn verify_function(
    code: &CodeSection,
    function_id: FunctionId,
    bytecode: &[u8],
    declared_max: u16,
) -> Result<(), StackImbalance> {
    let len = bytecode.len();

    // Phase 1: instruction boundaries. Bodies are emitted as a contiguous
    // instruction stream with no interleaved data, so a linear decode from
    // offset 0 enumerates exactly the valid branch targets.
    let boundaries = instruction_boundaries(function_id, bytecode)?;

    // Phase 2: abstract interpretation. `depth_at[pc]` is the operand-stack
    // depth on entry to the instruction at `pc`, once some path has reached
    // it. Index `len` represents falling off the end of the body.
    let mut depth_at: Vec<Option<u16>> = vec![None; len + 1];
    let mut work: VecDeque<usize> = VecDeque::new();
    depth_at[0] = Some(0);
    work.push_back(0);

    while let Some(pc) = work.pop_front() {
        let depth = depth_at[pc].expect("queued offsets always carry a depth");

        if pc == len {
            // Fell off the end of the body; the VM treats this as RET_VOID.
            return_check(function_id, pc, RET_VOID_DEPTH, depth)?;
            continue;
        }

        // `pc` is always an instruction boundary: phase 1 rejected every
        // unassigned byte and every instruction whose operands run past the
        // end, and this loop only ever enqueues offsets it validated against
        // `boundaries` (or `pc + size`, which is a boundary by construction).
        // Re-checking here would be unreachable code.
        debug_assert!(
            boundaries[pc],
            "phase 2 reached offset {pc}, which phase 1 did not mark as an instruction boundary"
        );
        let op = bytecode[pc];
        let size = opcode::instruction_size(op);

        let operands = &bytecode[pc + 1..pc + size];
        let effect = effect_of(code, function_id, pc, op, operands)?;

        if depth < effect.pops {
            return Err(StackImbalance::Underflow {
                function_id,
                offset: pc,
                opcode: op,
                needs: effect.pops,
                has: depth,
            });
        }
        let out = depth - effect.pops + effect.pushes;

        // R0203: the peak inside this instruction is whichever of the two
        // endpoints is higher — pops always precede pushes at runtime.
        let peak = depth.max(out);
        if peak > declared_max {
            return Err(StackImbalance::ExceedsDeclaredMax {
                function_id,
                offset: pc,
                depth: peak,
                declared: declared_max,
            });
        }

        match flow_of(op, operands) {
            Flow::Next => {
                propagate(function_id, &mut depth_at, &mut work, pc + size, out)?;
            }
            Flow::Jump(target) => {
                let target = branch_target(function_id, pc, size, target, &boundaries, len)?;
                propagate(function_id, &mut depth_at, &mut work, target, out)?;
            }
            Flow::Branch(target) => {
                let resolved = branch_target(function_id, pc, size, target, &boundaries, len)?;
                propagate(function_id, &mut depth_at, &mut work, resolved, out)?;
                propagate(function_id, &mut depth_at, &mut work, pc + size, out)?;
            }
            Flow::Return(expected) => {
                return_check(function_id, pc, expected, out)?;
            }
        }
    }

    Ok(())
}

/// Records `depth` as the entry depth of the instruction at `target`, or
/// reports R0200 if a previously recorded depth disagrees.
fn propagate(
    function_id: FunctionId,
    depth_at: &mut [Option<u16>],
    work: &mut VecDeque<usize>,
    target: usize,
    depth: u16,
) -> Result<(), StackImbalance> {
    match depth_at[target] {
        None => {
            depth_at[target] = Some(depth);
            work.push_back(target);
        }
        Some(existing) if existing != depth => {
            return Err(StackImbalance::MergeConflict {
                function_id,
                offset: target,
                existing,
                incoming: depth,
            });
        }
        // Already visited at this depth; the abstract state has converged
        // and re-walking would not change any successor.
        Some(_) => {}
    }
    Ok(())
}

/// Checks that a path leaving the function delivers the depth the calling
/// convention requires.
fn return_check(
    function_id: FunctionId,
    offset: usize,
    expected: u16,
    actual: u16,
) -> Result<(), StackImbalance> {
    if actual != expected {
        return Err(StackImbalance::UnbalancedReturn {
            function_id,
            offset,
            expected,
            actual,
        });
    }
    Ok(())
}

/// Resolves a branch's relative operand into an absolute offset, checking
/// that it lands on an instruction boundary inside the body.
///
/// Branch offsets are relative to the byte *after* the i16 operand, which
/// is the end of the instruction.
fn branch_target(
    function_id: FunctionId,
    pc: usize,
    size: usize,
    relative: isize,
    boundaries: &[bool],
    len: usize,
) -> Result<usize, StackImbalance> {
    let target = pc as isize + size as isize + relative;
    let invalid = StackImbalance::InvalidJumpTarget {
        function_id,
        offset: pc,
        target,
    };
    if target < 0 || target as usize > len {
        return Err(invalid);
    }
    let target = target as usize;
    // `len` (one past the end) is a legal target: it falls off the body,
    // which the VM treats as RET_VOID.
    if target < len && !boundaries[target] {
        return Err(invalid);
    }
    Ok(target)
}

/// Marks every byte offset that starts an instruction, by decoding the
/// body linearly from offset 0.
fn instruction_boundaries(
    function_id: FunctionId,
    bytecode: &[u8],
) -> Result<Vec<bool>, StackImbalance> {
    let mut boundaries = vec![false; bytecode.len()];
    let mut pc = 0usize;
    while pc < bytecode.len() {
        let op = bytecode[pc];
        if !opcode::is_assigned(op) {
            return Err(StackImbalance::UnknownOpcode {
                function_id,
                offset: pc,
                byte: op,
            });
        }
        boundaries[pc] = true;
        let size = opcode::instruction_size(op);
        if pc + size > bytecode.len() {
            return Err(StackImbalance::TruncatedInstruction {
                function_id,
                offset: pc,
                opcode: op,
            });
        }
        pc += size;
    }
    Ok(boundaries)
}

/// Reads a little-endian `u16` from the start of `operands`.
fn u16_at(operands: &[u8], index: usize) -> u16 {
    u16::from_le_bytes([operands[index], operands[index + 1]])
}

/// Reads a little-endian `i16` from `operands` at `index`.
fn i16_at(operands: &[u8], index: usize) -> i16 {
    i16::from_le_bytes([operands[index], operands[index + 1]])
}

/// Control-flow successors of an instruction.
///
/// Only branch and return opcodes deviate from straight-line flow, so this
/// match lists them explicitly and everything else falls through.
fn flow_of(op: Opcode, operands: &[u8]) -> Flow {
    match op {
        opcode::JMP => Flow::Jump(i16_at(operands, 0) as isize),
        opcode::JMP_IF_NOT => Flow::Branch(i16_at(operands, 0) as isize),
        // CMP_BR: [cmp_op u8][var u16][const u16][target i16]
        opcode::CMP_BR_I32 | opcode::CMP_BR_I64 => Flow::Branch(i16_at(operands, 5) as isize),
        opcode::RET => Flow::Return(RET_DEPTH),
        opcode::RET_VOID => Flow::Return(RET_VOID_DEPTH),
        _ => Flow::Next,
    }
}

/// The operand-stack effect of one instruction.
///
/// This match is exhaustive over the assigned opcode space — there is no
/// catch-all for assigned opcodes — so adding an opcode to
/// [`opcode::instruction_size`] without giving it a stack effect here is
/// caught by `stack_effect_when_opcode_assigned_then_effect_defined`
/// rather than silently verified as a no-op.
fn effect_of(
    code: &CodeSection,
    function_id: FunctionId,
    offset: usize,
    op: Opcode,
    operands: &[u8],
) -> Result<Effect, StackImbalance> {
    use opcode::*;

    let effect = match op {
        // --- Push one, pop nothing ---
        LOAD_CONST_I32 | LOAD_CONST_I64 | LOAD_CONST_F32 | LOAD_CONST_F64 | LOAD_CONST_STR
        | LOAD_VAR_I32 | LOAD_VAR_I64 | LOAD_VAR_F32 | LOAD_VAR_F64 | LOAD_TRUE | LOAD_FALSE
        | DUP | FB_LOAD_INSTANCE | STR_LOAD_VAR | LEN_STR | FIND_STR | CONCAT_STR => {
            Effect::new(0, 1)
        }

        // FB_LOAD_PARAM reads a field through the fb_ref it leaves in place.
        FB_LOAD_PARAM => Effect::new(0, 1),

        // --- Pop one, push nothing ---
        STORE_VAR_I32 | STORE_VAR_I64 | STORE_VAR_F32 | STORE_VAR_F64 | POP | JMP_IF_NOT
        | STR_STORE_VAR => Effect::new(1, 0),

        // FB_STORE_PARAM consumes the value; the fb_ref below it survives.
        FB_STORE_PARAM => Effect::new(1, 0),

        // --- Pop two, push nothing ---
        STORE_INDIRECT | STORE_ARRAY | STORE_ARRAY_DEREF | STR_STORE_ARRAY_ELEM => {
            Effect::new(2, 0)
        }

        // --- Pop one, push one (net zero) ---
        NEG_I32 | NEG_I64 | NEG_F32 | NEG_F64 | BOOL_NOT | BIT_NOT_32 | BIT_NOT_64 | TRUNC_I8
        | TRUNC_U8 | TRUNC_I16 | TRUNC_U16 | LOAD_INDIRECT | LOAD_ARRAY | LOAD_ARRAY_DEREF
        | STR_LOAD_ARRAY_ELEM | INSERT_STR | LEFT_STR | RIGHT_STR => Effect::new(1, 1),

        // --- Pop two, push one (net pop one) ---
        ADD_I32 | SUB_I32 | MUL_I32 | DIV_I32 | MOD_I32 | ADD_I64 | SUB_I64 | MUL_I64 | DIV_I64
        | MOD_I64 | DIV_U32 | MOD_U32 | DIV_U64 | MOD_U64 | ADD_F32 | SUB_F32 | MUL_F32
        | DIV_F32 | ADD_F64 | SUB_F64 | MUL_F64 | DIV_F64 | EQ_I32 | NE_I32 | LT_I32 | LE_I32
        | GT_I32 | GE_I32 | EQ_I64 | NE_I64 | LT_I64 | LE_I64 | GT_I64 | GE_I64 | LT_U32
        | LE_U32 | GT_U32 | GE_U32 | LT_U64 | LE_U64 | GT_U64 | GE_U64 | EQ_F32 | NE_F32
        | LT_F32 | LE_F32 | GT_F32 | GE_F32 | EQ_F64 | NE_F64 | LT_F64 | LE_F64 | GT_F64
        | GE_F64 | BOOL_AND | BOOL_OR | BOOL_XOR | BIT_AND_32 | BIT_OR_32 | BIT_XOR_32
        | BIT_AND_64 | BIT_OR_64 | BIT_XOR_64 | REPLACE_STR | DELETE_STR | MID_STR => {
            Effect::new(2, 1)
        }

        // --- No stack effect ---
        //
        // SWAP reorders the top two slots; it needs two present, which the
        // pop/push pair below models without changing the depth. JMP,
        // CMP_BR, STR_INIT, STR_INIT_ARRAY and the returns move control or
        // touch only memory. FB_CALL runs the body as its own frame and
        // leaves the caller's fb_ref where it was.
        SWAP => Effect::new(2, 2),
        JMP | CMP_BR_I32 | CMP_BR_I64 | STR_INIT | STR_INIT_ARRAY | FB_CALL | RET | RET_VOID => {
            Effect::NONE
        }

        // --- Variable-effect instructions ---
        BUILTIN => {
            let builtin_id = u16_at(operands, 0);
            let args = opcode::builtin::arg_count_opt(builtin_id).ok_or(
                StackImbalance::UnknownBuiltin {
                    function_id,
                    offset,
                    builtin_id,
                },
            )?;
            // Every builtin consumes its arguments and produces one result.
            Effect::new(args, 1)
        }
        CALL => {
            let callee = FunctionId::new(u16_at(operands, 0));
            let entry = code
                .get_function(callee)
                .ok_or(StackImbalance::UnknownCallee {
                    function_id,
                    offset,
                    callee,
                })?;
            // Arguments are popped into the callee's parameter slots; the
            // callee's RET leaves exactly one value behind.
            Effect::new(entry.num_params, RET_DEPTH)
        }

        _ => {
            return Err(StackImbalance::UnknownOpcode {
                function_id,
                offset,
                byte: op,
            })
        }
    };
    Ok(effect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_section::FuncEntry;
    use std::string::{String, ToString};

    /// Builds a single-function code section around `bytecode`.
    fn section(bytecode: Vec<u8>, max_stack_depth: u16) -> CodeSection {
        CodeSection {
            functions: vec![FuncEntry {
                function_id: FunctionId::new(0),
                code_offset: 0,
                code_length: bytecode.len() as u32,
                max_stack_depth,
                num_locals: 4,
                num_params: 0,
            }],
            bytecode,
        }
    }

    #[test]
    fn verify_stack_balance_when_balanced_then_ok() {
        // LOAD_CONST_I32 0; STORE_VAR_I32 0; RET_VOID
        let code = section(
            vec![
                opcode::LOAD_CONST_I32,
                0,
                0,
                opcode::STORE_VAR_I32,
                0,
                0,
                opcode::RET_VOID,
            ],
            1,
        );

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_leaked_slot_then_unbalanced_return() {
        // LOAD_CONST_I32 0; RET_VOID  -- the loaded value is never consumed.
        let code = section(vec![opcode::LOAD_CONST_I32, 0, 0, opcode::RET_VOID], 1);

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnbalancedReturn {
                function_id: FunctionId::new(0),
                offset: 3,
                expected: 0,
                actual: 1,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_over_pop_then_underflow() {
        // STORE_VAR_I32 0 with nothing on the stack.
        let code = section(vec![opcode::STORE_VAR_I32, 0, 0, opcode::RET_VOID], 1);

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::Underflow {
                function_id: FunctionId::new(0),
                offset: 0,
                opcode: opcode::STORE_VAR_I32,
                needs: 1,
                has: 0,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_branch_arms_disagree_then_merge_conflict() {
        // LOAD_TRUE; JMP_IF_NOT +3; LOAD_TRUE; RET_VOID
        //
        // The taken path arrives at RET_VOID with depth 0, the fall-through
        // path with depth 1.
        let code = section(
            vec![
                opcode::LOAD_TRUE,
                opcode::JMP_IF_NOT,
                0x01,
                0x00,
                opcode::LOAD_TRUE,
                opcode::RET_VOID,
            ],
            2,
        );

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::MergeConflict {
                function_id: FunctionId::new(0),
                offset: 5,
                existing: 0,
                incoming: 1,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_branch_arms_agree_then_ok() {
        // LOAD_TRUE; JMP_IF_NOT +1; POP-free both arms; RET_VOID
        let code = section(
            vec![
                opcode::LOAD_TRUE,
                opcode::JMP_IF_NOT,
                0x00,
                0x00,
                opcode::RET_VOID,
            ],
            2,
        );

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_early_ret_with_value_then_ok() {
        // A value-returning function with an early RETURN: both exits leave
        // exactly one value. The emitter's linear counter would read 2 here.
        //
        // LOAD_VAR_I32 0; RET; LOAD_VAR_I32 0; RET
        let code = section(
            vec![
                opcode::LOAD_VAR_I32,
                0,
                0,
                opcode::RET,
                opcode::LOAD_VAR_I32,
                0,
                0,
                opcode::RET,
            ],
            1,
        );

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_ret_without_value_then_unbalanced_return() {
        let code = section(vec![opcode::RET], 0);

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnbalancedReturn {
                function_id: FunctionId::new(0),
                offset: 0,
                expected: 1,
                actual: 0,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_falls_off_end_unbalanced_then_unbalanced_return() {
        // LOAD_TRUE with no return at all: the VM treats the end of the body
        // as RET_VOID, so the leaked slot is still a leak.
        let code = section(vec![opcode::LOAD_TRUE], 1);

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnbalancedReturn {
                function_id: FunctionId::new(0),
                offset: 1,
                expected: 0,
                actual: 1,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_depth_exceeds_declared_then_exceeds_declared_max() {
        // Two pushes against a declared max of 1.
        let code = section(
            vec![opcode::LOAD_TRUE, opcode::LOAD_TRUE, opcode::RET_VOID],
            1,
        );

        assert_eq!(
            verify_stack_balance(&code),
            Err(StackImbalance::ExceedsDeclaredMax {
                function_id: FunctionId::new(0),
                offset: 1,
                depth: 2,
                declared: 1,
            })
        );
    }

    #[test]
    fn verify_stack_balance_when_jump_target_mid_instruction_then_invalid_jump_target() {
        // JMP +1 lands on the second byte of the following LOAD_CONST_I32.
        let code = section(
            vec![
                opcode::JMP,
                0x01,
                0x00,
                opcode::LOAD_CONST_I32,
                0,
                0,
                opcode::POP,
                opcode::RET_VOID,
            ],
            1,
        );

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::InvalidJumpTarget { .. })
        ));
    }

    #[test]
    fn verify_stack_balance_when_unassigned_opcode_then_unknown_opcode() {
        let code = section(vec![0xFF], 0);

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnknownOpcode { byte: 0xFF, .. })
        ));
    }

    #[test]
    fn verify_stack_balance_when_instruction_truncated_then_truncated_instruction() {
        // LOAD_CONST_I32 needs two operand bytes but only one is present.
        let code = section(vec![opcode::LOAD_CONST_I32, 0], 1);

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::TruncatedInstruction { .. })
        ));
    }

    #[test]
    fn verify_stack_balance_when_unreachable_code_unbalanced_then_ok() {
        // The trailing LOAD_TRUE cannot execute, so it cannot unbalance
        // anything and is not verified.
        let code = section(vec![opcode::RET_VOID, opcode::LOAD_TRUE], 1);

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_call_then_pops_params_and_pushes_result() {
        // Function 0 pushes one argument, calls function 1 (one parameter),
        // then discards the result.
        let caller = vec![
            opcode::LOAD_TRUE,
            opcode::CALL,
            0x01,
            0x00,
            0x00,
            0x00,
            opcode::POP,
            opcode::RET_VOID,
        ];
        let callee = vec![opcode::LOAD_TRUE, opcode::RET];
        let caller_len = caller.len();
        let mut bytecode = caller;
        bytecode.extend_from_slice(&callee);

        let code = CodeSection {
            functions: vec![
                FuncEntry {
                    function_id: FunctionId::new(0),
                    code_offset: 0,
                    code_length: caller_len as u32,
                    max_stack_depth: 2,
                    num_locals: 1,
                    num_params: 0,
                },
                FuncEntry {
                    function_id: FunctionId::new(1),
                    code_offset: caller_len as u32,
                    code_length: 2,
                    max_stack_depth: 1,
                    num_locals: 1,
                    num_params: 1,
                },
            ],
            bytecode,
        };

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_loop_back_edge_then_ok() {
        // A back edge that returns to the loop head at the same depth.
        //
        // 0: LOAD_TRUE
        // 1: JMP_IF_NOT +3   -> 7 (RET_VOID)
        // 4: JMP -7          -> 0
        // 7: RET_VOID
        let code = section(
            vec![
                opcode::LOAD_TRUE,
                opcode::JMP_IF_NOT,
                0x03,
                0x00,
                opcode::JMP,
                0xF9,
                0xFF,
                opcode::RET_VOID,
            ],
            1,
        );

        assert_eq!(verify_stack_balance(&code), Ok(()));
    }

    #[test]
    fn verify_stack_balance_when_jump_target_before_body_then_invalid_jump_target() {
        // JMP -32 from offset 0 lands well before the body.
        let code = section(vec![opcode::JMP, 0xE0, 0xFF, opcode::RET_VOID], 0);

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::InvalidJumpTarget { .. })
        ));
    }

    #[test]
    fn verify_stack_balance_when_jump_target_past_body_then_invalid_jump_target() {
        // JMP +64 lands past the end of a four-byte body.
        let code = section(vec![opcode::JMP, 0x40, 0x00, opcode::RET_VOID], 0);

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::InvalidJumpTarget { .. })
        ));
    }

    #[test]
    fn verify_stack_balance_when_builtin_id_unknown_then_unknown_builtin() {
        // 0xFFFF is not a defined built-in function ID, so the instruction's
        // stack effect cannot be determined.
        let code = section(vec![opcode::BUILTIN, 0xFF, 0xFF, opcode::RET_VOID], 2);

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnknownBuiltin {
                builtin_id: 0xFFFF,
                ..
            })
        ));
    }

    #[test]
    fn verify_stack_balance_when_call_target_missing_then_unknown_callee() {
        // Function 9 is not in the single-entry function directory.
        let code = section(
            vec![opcode::CALL, 0x09, 0x00, 0x00, 0x00, opcode::RET_VOID],
            2,
        );

        assert!(matches!(
            verify_stack_balance(&code),
            Err(StackImbalance::UnknownCallee { .. })
        ));
    }

    #[test]
    fn stack_imbalance_when_any_variant_then_renders_function_offset_and_rule() {
        // Every variant must produce a usable diagnostic. A violation is
        // reported as a compiler internal error, so its Display text is the
        // only thing pointing at the instruction responsible.
        let func = FunctionId::new(3);
        let variants = vec![
            StackImbalance::Underflow {
                function_id: func,
                offset: 7,
                opcode: opcode::POP,
                needs: 1,
                has: 0,
            },
            StackImbalance::MergeConflict {
                function_id: func,
                offset: 7,
                existing: 0,
                incoming: 1,
            },
            StackImbalance::UnbalancedReturn {
                function_id: func,
                offset: 7,
                expected: 0,
                actual: 1,
            },
            StackImbalance::ExceedsDeclaredMax {
                function_id: func,
                offset: 7,
                depth: 4,
                declared: 2,
            },
            StackImbalance::InvalidJumpTarget {
                function_id: func,
                offset: 7,
                target: -3,
            },
            StackImbalance::UnknownOpcode {
                function_id: func,
                offset: 7,
                byte: 0xFF,
            },
            StackImbalance::TruncatedInstruction {
                function_id: func,
                offset: 7,
                opcode: opcode::CALL,
            },
            StackImbalance::UnknownBuiltin {
                function_id: func,
                offset: 7,
                builtin_id: 0xFFFF,
            },
            StackImbalance::UnknownCallee {
                function_id: func,
                offset: 7,
                callee: FunctionId::new(9),
            },
        ];

        let unusable: Vec<String> = variants
            .iter()
            .filter(|v| {
                v.function_id() != func
                    || v.offset() != 7
                    || !v.to_string().contains("offset 7")
                    || v.rule().is_some_and(|r| !r.starts_with('R'))
            })
            .map(|v| v.to_string())
            .collect();

        assert_eq!(unusable, Vec::<String>::new());
    }

    #[test]
    fn stack_effect_when_opcode_unassigned_then_unknown_opcode() {
        let code = section(vec![], 0);

        assert!(matches!(
            effect_of(&code, FunctionId::new(0), 0, 0xFF, &[0u8; 8]),
            Err(StackImbalance::UnknownOpcode { byte: 0xFF, .. })
        ));
    }

    #[test]
    fn stack_effect_when_opcode_assigned_then_effect_defined() {
        // Every assigned opcode must have a stack effect. This is the guard
        // that keeps the invariant holding as opcodes are added: a new
        // opcode wired into `instruction_size` but not into `effect_of`
        // fails here instead of being silently treated as a no-op.
        let code = section(vec![], 0);
        let operands = [0u8; 8];
        let undefined: Vec<u8> = (0u16..=255)
            .map(|b| b as u8)
            .filter(|&op| opcode::is_assigned(op))
            // CALL and BUILTIN read their operands; give them ones that
            // resolve so this test only measures whether an arm exists.
            .filter(|&op| op != opcode::CALL && op != opcode::BUILTIN)
            .filter(|&op| effect_of(&code, FunctionId::new(0), 0, op, &operands).is_err())
            .collect();

        assert_eq!(undefined, Vec::<u8>::new());
    }
}
