//! Verifies that a container's declared temp string buffer pool is large
//! enough for its own bytecode (verifier rule R0204).
//!
//! # This is a backstop around a design we would rather not need
//!
//! The pool exists because the VM allocates nothing at run time
//! (ADR-0010), so its size has to be decided when the program is compiled.
//! Any number decided at compile time can be decided wrongly, and all this
//! pass does is make a wrong one loud — at compile time, naming the
//! instruction — instead of quiet until a `V9009` trap in the field.
//!
//! **The design that removes the risk entirely is to have no pool.** String
//! operations already spill *nested* results into data-region scratch slots
//! chosen at compile time; that is why `CONCAT(CONCAT(a, b), c)` needs one
//! buffer rather than two. If a string operation's own result went to a
//! compile-time scratch slot as well, rather than to a slot bump-allocated
//! from a shared pool at run time, then `num_temp_bufs`,
//! `max_temp_buf_bytes`, the allocator, its watermark, `V9009` and this
//! module would all stop existing, and a string result would be addressed
//! the way every other piece of data-region storage already is: by an
//! offset fixed when the instruction was emitted.
//!
//! That change alters the meaning of `buf_idx` across the instruction set,
//! so it is not this pass's job. But it is the fix. Treat what follows as a
//! guard rail, not as ground to build on: work that makes this module
//! unnecessary is worth more than work that extends it.
//!
//! # Model
//!
//! Temp buffers form a stack. An allocating instruction takes the next
//! slot; `STR_STORE_VAR` and `STR_STORE_ARRAY_ELEM` hand a slot back once
//! they have copied its contents into the data region (ADR-0052). So the
//! question "does the pool overflow" is "how deep does that stack get",
//! which is a path property, decided here by abstract interpretation over
//! each function's control-flow graph — over the bytecode in the container,
//! not over anything the compiler remembers about emitting it. That
//! independence is the whole point: codegen computes the same number from
//! its own bookkeeping, and a check that consulted that bookkeeping would
//! re-confirm codegen's arithmetic rather than test its output.
//!
//! Two things differ from the operand-stack walk in [`crate::verify`]:
//!
//! - **A merge takes the maximum rather than conflicting.** Operand-stack
//!   depth must agree at a merge because the calling convention depends on
//!   it. Temp depth may legitimately differ between arms — one may leave a
//!   string for the caller and the other not — and what this rule needs is
//!   the worst case, so a merge that raises the recorded depth re-enqueues
//!   its successors.
//! - **Termination comes from the bound, not from convergence.** A loop
//!   body with net-positive temp depth has no fixpoint: the depth grows on
//!   every trip. That is precisely the defect this rule exists to catch, and
//!   the walk fails as soon as the depth passes the declared pool, so such
//!   bytecode terminates the walk instead of spinning it.
//!
//! A callee's buffers sit on top of whatever its caller holds live, so the
//! per-function maxima are composed along the call graph — derived here
//! from the `CALL` / `FB_CALL` / `METHOD_CALL` instructions themselves —
//! and the heaviest path is what the declared pool must cover.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::vec;
use std::vec::Vec;

use crate::cfg::{self, CfgError, Flow};
use crate::code_section::CodeSection;
use crate::id_types::FunctionId;
use crate::opcode::{self, Opcode};
use crate::type_section::UserFbDescriptor;

/// A container whose declared temp buffer pool cannot cover its bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TempBufferOverrun {
    /// R0204: some path needs more temp buffers than the header declares.
    ExceedsDeclaredPool {
        /// The function whose body reaches the excessive depth.
        function_id: FunctionId,
        /// Byte offset, within that body, of the allocating instruction.
        offset: usize,
        /// Buffers live at that point, including the callers' share.
        depth: u16,
        /// `num_temp_bufs` from the container header.
        declared: u16,
    },
    /// The call graph derived from the bytecode contains a cycle, so no
    /// heaviest path exists. Recursion is rejected earlier by semantic
    /// analysis; this is a fail-loud backstop against that regressing.
    RecursiveCallGraph { function_id: FunctionId },
    /// The body does not decode into a control-flow graph. `verify_stack_balance`
    /// reports the same bytes in its own vocabulary; this variant exists so
    /// this pass never walks a body it cannot read.
    Undecodable { function_id: FunctionId },
}

impl TempBufferOverrun {
    /// The verifier rule this violation belongs to.
    pub fn rule(&self) -> Option<&'static str> {
        match self {
            TempBufferOverrun::ExceedsDeclaredPool { .. } => Some("R0204"),
            _ => None,
        }
    }
}

impl fmt::Display for TempBufferOverrun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TempBufferOverrun::ExceedsDeclaredPool {
                function_id,
                offset,
                depth,
                declared,
            } => write!(
                f,
                "function {function_id} at offset {offset} needs {depth} temporary string \
                 buffers but the container declares {declared}"
            ),
            TempBufferOverrun::RecursiveCallGraph { function_id } => write!(
                f,
                "call graph cycle involving function {function_id}; the temp buffer bound \
                 has no heaviest path"
            ),
            TempBufferOverrun::Undecodable { function_id } => {
                write!(f, "function {function_id} does not decode")
            }
        }
    }
}

/// How one instruction changes the number of temp buffers held live.
///
/// This match names every opcode that touches the pool. Anything else is
/// zero, which is safe by construction: an opcode that neither pushes nor
/// consumes a `buf_idx` cannot change the watermark.
fn temp_effect(op: Opcode) -> i8 {
    use opcode::*;
    match op {
        // Produce a value in a fresh buffer and push its index.
        LOAD_CONST_STR | STR_LOAD_VAR | STR_LOAD_ARRAY_ELEM | CONCAT_STR | LEFT_STR | RIGHT_STR
        | MID_STR | INSERT_STR | DELETE_STR | REPLACE_STR => 1,
        // Copy a buffer's contents out and hand the slot back.
        STR_STORE_VAR | STR_STORE_ARRAY_ELEM => -1,
        _ => 0,
    }
}

/// Whether a `BUILTIN` writes its result into a fresh temp buffer.
fn builtin_allocates(func_id: u16) -> bool {
    opcode::builtin::allocates_temp_buf(func_id)
}

/// Verifies R0204 for every function reachable from any of `entries`.
///
/// `entries` are the roots the VM can start a call chain from: each
/// program's scan function and its init function. A function no root
/// reaches never runs, so it cannot draw on the pool and is not charged to
/// it. `declared` is the header's `num_temp_bufs`. `fb_types` resolves a
/// `FB_CALL`, which names a *type*, to the body it enters.
pub fn verify_temp_buffer_bound(
    code: &CodeSection,
    fb_types: &[UserFbDescriptor],
    entries: &[FunctionId],
    declared: u16,
) -> Result<(), TempBufferOverrun> {
    let mut per_function: HashMap<FunctionId, FunctionUsage> = HashMap::new();
    for function in &code.functions {
        let bytecode = code
            .get_function_bytecode(function.function_id)
            .unwrap_or_default();
        per_function.insert(
            function.function_id,
            walk_function(function.function_id, bytecode, fb_types, declared)?,
        );
    }

    // Compose along the call graph: a callee's buffers sit on top of its
    // caller's, so the pool must cover the heaviest path from every root.
    for entry in entries {
        heaviest_path(&per_function, *entry, declared)?;
    }
    Ok(())
}

/// One function's own draw on the pool, plus who it calls.
struct FunctionUsage {
    /// Most buffers this body holds live at once, ignoring its callees.
    max_depth: u16,
    /// Offset of the instruction that reaches `max_depth`, for reporting.
    peak_offset: usize,
    /// Functions this body enters, in first-encountered bytecode order.
    /// A list rather than a set so the walk below is deterministic, and so
    /// a reported cycle names the same function run to run.
    callees: Vec<FunctionId>,
}

/// Walks one body's CFG, returning its peak temp depth and its callees.
/// `declared` is used only to bound the exploration, not to judge the
/// body: see the comment at the cap below.
fn walk_function(
    function_id: FunctionId,
    bytecode: &[u8],
    fb_types: &[UserFbDescriptor],
    declared: u16,
) -> Result<FunctionUsage, TempBufferOverrun> {
    let len = bytecode.len();
    let boundaries = cfg::instruction_boundaries(bytecode)
        .map_err(|_| TempBufferOverrun::Undecodable { function_id })?;

    // `depth_at[pc]` is the greatest temp depth any path has delivered to
    // the instruction at `pc`. Index `len` is falling off the end.
    let mut depth_at: Vec<Option<u16>> = vec![None; len + 1];
    let mut work: VecDeque<usize> = VecDeque::new();
    let mut callees: Vec<FunctionId> = Vec::new();
    let mut max_depth = 0u16;
    let mut peak_offset = 0usize;

    depth_at[0] = Some(0);
    work.push_back(0);

    while let Some(pc) = work.pop_front() {
        let depth = depth_at[pc].expect("queued offsets always carry a depth");
        if pc == len {
            continue;
        }

        let op = bytecode[pc];
        let size = opcode::instruction_size(op);
        if pc + size > len {
            return Err(TempBufferOverrun::Undecodable { function_id });
        }
        let operands = &bytecode[pc + 1..pc + size];

        record_callee(op, operands, fb_types, &mut callees);

        let allocates = temp_effect(op) > 0
            || (op == opcode::BUILTIN && builtin_allocates(cfg::u16_at(operands, 0)));
        let releases = temp_effect(op) < 0;

        let out = if allocates {
            let raised = depth.saturating_add(1);
            if raised > max_depth {
                max_depth = raised;
                peak_offset = pc;
            }
            // Stop exploring past the declared bound rather than reporting
            // here. Stopping is what makes a loop that nets positive
            // terminate: its depth has no fixpoint, so without a cap the
            // walk would raise it forever. Reporting is left to the
            // call-graph pass, which alone knows whether this function is
            // reachable -- a body no entry point can call never runs, so it
            // must not be charged to the pool.
            if raised > declared {
                continue;
            }
            raised
        } else if releases {
            // Saturating, as the VM's allocator is: a body may consume a
            // buffer its caller produced (a STRING-returning call).
            depth.saturating_sub(1)
        } else {
            depth
        };

        match cfg::flow_of(op, operands) {
            Flow::Next => raise(&mut depth_at, &mut work, pc + size, out),
            Flow::Jump(rel) => {
                let target = resolve(function_id, pc, size, rel, &boundaries, len)?;
                raise(&mut depth_at, &mut work, target, out);
            }
            Flow::Branch(rel) => {
                let target = resolve(function_id, pc, size, rel, &boundaries, len)?;
                raise(&mut depth_at, &mut work, target, out);
                raise(&mut depth_at, &mut work, pc + size, out);
            }
            Flow::Return => {}
        }
    }

    Ok(FunctionUsage {
        max_depth,
        peak_offset,
        callees,
    })
}

/// Records `depth` at `target` if it is deeper than anything seen there,
/// re-enqueuing so the increase reaches the successors.
///
/// Unlike the operand-stack walk this never reports a merge conflict: arms
/// that deliver different temp depths are legal, and the bound needs the
/// larger.
fn raise(depth_at: &mut [Option<u16>], work: &mut VecDeque<usize>, target: usize, depth: u16) {
    match depth_at[target] {
        Some(existing) if existing >= depth => {}
        _ => {
            depth_at[target] = Some(depth);
            work.push_back(target);
        }
    }
}

fn resolve(
    function_id: FunctionId,
    pc: usize,
    size: usize,
    relative: isize,
    boundaries: &[bool],
    len: usize,
) -> Result<usize, TempBufferOverrun> {
    cfg::branch_target(pc, size, relative, boundaries, len).map_err(|e| match e {
        CfgError::InvalidJumpTarget { .. }
        | CfgError::UnknownOpcode { .. }
        | CfgError::TruncatedInstruction { .. } => TempBufferOverrun::Undecodable { function_id },
    })
}

/// Adds the function a call instruction enters, if it is one.
///
/// `FB_CALL` names a *type*, so the body it enters comes from the container's
/// user-FB descriptors; an intrinsic function block has no descriptor and no
/// PLC body, and never touches the pool.
fn record_callee(
    op: Opcode,
    operands: &[u8],
    fb_types: &[UserFbDescriptor],
    callees: &mut Vec<FunctionId>,
) {
    let callee = match op {
        opcode::CALL | opcode::METHOD_CALL => Some(FunctionId::new(cfg::u16_at(operands, 0))),
        opcode::FB_CALL => {
            let type_id = cfg::u16_at(operands, 0);
            fb_types
                .iter()
                .find(|d| d.type_id.raw() == type_id)
                .map(|d| d.function_id)
        }
        _ => None,
    };
    if let Some(callee) = callee {
        if !callees.contains(&callee) {
            callees.push(callee);
        }
    }
}

/// Heaviest path from `entry`, each function weighted by its own peak.
///
/// Mirrors the call-depth walk: a three-colour DFS with an explicit stack,
/// reporting a cycle rather than looping on one.
fn heaviest_path(
    per_function: &HashMap<FunctionId, FunctionUsage>,
    entry: FunctionId,
    declared: u16,
) -> Result<(), TempBufferOverrun> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Colour {
        Grey,
        Black,
    }

    struct Frame {
        node: FunctionId,
        children: Vec<FunctionId>,
        next: usize,
        deepest_child: u16,
    }

    let children_of = |f: FunctionId| -> Vec<FunctionId> {
        per_function
            .get(&f)
            .map(|u| u.callees.clone())
            .unwrap_or_default()
    };
    let weight_of = |f: FunctionId| -> u16 { per_function.get(&f).map_or(0, |u| u.max_depth) };

    let mut colour: HashMap<FunctionId, Colour> = HashMap::new();
    let mut total: HashMap<FunctionId, u16> = HashMap::new();
    let mut stack = vec![Frame {
        node: entry,
        children: children_of(entry),
        next: 0,
        deepest_child: 0,
    }];
    colour.insert(entry, Colour::Grey);

    while let Some(top) = stack.last_mut() {
        if top.next < top.children.len() {
            let child = top.children[top.next];
            top.next += 1;
            match colour.get(&child).copied() {
                Some(Colour::Grey) => {
                    return Err(TempBufferOverrun::RecursiveCallGraph { function_id: child })
                }
                Some(Colour::Black) => {
                    let d = total[&child];
                    if d > top.deepest_child {
                        top.deepest_child = d;
                    }
                }
                None => {
                    colour.insert(child, Colour::Grey);
                    let grandchildren = children_of(child);
                    stack.push(Frame {
                        node: child,
                        children: grandchildren,
                        next: 0,
                        deepest_child: 0,
                    });
                }
            }
        } else {
            let node = top.node;
            let depth = top.deepest_child.saturating_add(weight_of(node));
            if depth > declared {
                let usage = per_function.get(&node);
                return Err(TempBufferOverrun::ExceedsDeclaredPool {
                    function_id: node,
                    offset: usage.map_or(0, |u| u.peak_offset),
                    depth,
                    declared,
                });
            }
            total.insert(node, depth);
            colour.insert(node, Colour::Black);
            stack.pop();
            if let Some(parent) = stack.last_mut() {
                if depth > parent.deepest_child {
                    parent.deepest_child = depth;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_section::FuncEntry;

    const ENTRY: FunctionId = FunctionId::INIT;

    /// One function at id 0, which is also the entry.
    fn section(bytecode: Vec<u8>) -> CodeSection {
        CodeSection {
            functions: vec![FuncEntry {
                function_id: FunctionId::new(0),
                code_offset: 0,
                code_length: bytecode.len() as u32,
                max_stack_depth: 8,
                num_locals: 4,
                num_params: 0,
            }],
            bytecode,
        }
    }

    fn verify(code: &CodeSection, declared: u16) -> Result<(), TempBufferOverrun> {
        verify_temp_buffer_bound(code, &[], &[ENTRY], declared)
    }

    /// `CONCAT_STR; STR_STORE_VAR` — allocate one, hand it back.
    fn concat_then_store() -> Vec<u8> {
        let mut v = vec![opcode::CONCAT_STR];
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.push(opcode::STR_STORE_VAR);
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    }

    #[test]
    fn verify_temp_buffer_bound_when_pool_covers_bytecode_then_ok() {
        let mut body = concat_then_store();
        body.push(opcode::RET_VOID);

        assert_eq!(verify(&section(body), 1), Ok(()));
    }

    #[test]
    fn verify_temp_buffer_bound_when_pool_is_one_too_small_then_r0204() {
        let mut body = concat_then_store();
        body.push(opcode::RET_VOID);

        let result = verify(&section(body), 0);

        assert!(matches!(
            result,
            Err(TempBufferOverrun::ExceedsDeclaredPool {
                depth: 1,
                declared: 0,
                ..
            })
        ));
    }

    #[test]
    fn verify_temp_buffer_bound_when_violation_then_names_rule_r0204() {
        let mut body = concat_then_store();
        body.push(opcode::RET_VOID);

        let err = verify(&section(body), 0).unwrap_err();

        assert_eq!(err.rule(), Some("R0204"));
    }

    /// Two allocations live at once need two slots. This is the shape the
    /// rule has to size for, as distinct from the same site running twice.
    #[test]
    fn verify_temp_buffer_bound_when_two_live_at_once_then_needs_two() {
        let mut body = vec![opcode::CONCAT_STR];
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(opcode::CONCAT_STR);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(opcode::STR_STORE_VAR);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(opcode::STR_STORE_VAR);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(opcode::RET_VOID);

        assert!(verify(&section(body.clone()), 1).is_err());
        assert_eq!(verify(&section(body), 2), Ok(()));
    }

    /// The shape that caused #1590: a string operation whose result is
    /// consumed each time round. Running it a million times still needs one
    /// buffer, so the declared pool of 1 must be accepted.
    #[test]
    fn verify_temp_buffer_bound_when_balanced_loop_then_one_buffer_suffices() {
        // body: CONCAT_STR; STR_STORE_VAR; JMP back to 0; RET_VOID
        let mut body = concat_then_store();
        let back = -((body.len() + 3) as i16);
        body.push(opcode::JMP);
        body.extend_from_slice(&back.to_le_bytes());
        body.push(opcode::RET_VOID);

        assert_eq!(verify(&section(body), 1), Ok(()));
    }

    /// A loop body that allocates without consuming has no fixpoint: depth
    /// grows every trip. The walk must terminate by failing against the
    /// bound rather than spinning.
    #[test]
    fn verify_temp_buffer_bound_when_loop_leaks_a_buffer_then_terminates_with_r0204() {
        // body: CONCAT_STR (no store); JMP back to 0
        let mut body = vec![opcode::CONCAT_STR];
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        let back = -((body.len() + 3) as i16);
        body.push(opcode::JMP);
        body.extend_from_slice(&back.to_le_bytes());

        let result = verify(&section(body), 4);

        assert!(matches!(
            result,
            Err(TempBufferOverrun::ExceedsDeclaredPool { declared: 4, .. })
        ));
    }

    /// Branch arms may leave different temp depths; the bound takes the
    /// larger rather than reporting a merge conflict the way the
    /// operand-stack rule does.
    #[test]
    fn verify_temp_buffer_bound_when_arms_differ_then_takes_the_deeper() {
        // LOAD_TRUE; JMP_IF_NOT over CONCAT_STR; CONCAT_STR; STR_STORE_VAR; RET_VOID
        let mut body = vec![opcode::LOAD_TRUE, opcode::JMP_IF_NOT];
        body.extend_from_slice(&9i16.to_le_bytes());
        body.extend_from_slice(&concat_then_store());
        body.push(opcode::RET_VOID);

        assert_eq!(verify(&section(body.clone()), 1), Ok(()));
        assert!(verify(&section(body), 0).is_err());
    }

    #[test]
    fn verify_temp_buffer_bound_when_builtin_allocates_then_counted() {
        let mut body = vec![opcode::BUILTIN];
        body.extend_from_slice(&opcode::builtin::CONV_I32_TO_STR.to_le_bytes());
        body.push(opcode::RET_VOID);

        assert!(verify(&section(body.clone()), 0).is_err());
        assert_eq!(verify(&section(body), 1), Ok(()));
    }

    #[test]
    fn verify_temp_buffer_bound_when_builtin_does_not_allocate_then_not_counted() {
        let mut body = vec![opcode::BUILTIN];
        body.extend_from_slice(&opcode::builtin::CMP_STR.to_le_bytes());
        body.push(opcode::RET_VOID);

        assert_eq!(verify(&section(body), 0), Ok(()));
    }

    /// A callee's buffers sit on top of its caller's, so a call chain needs
    /// the sum. Function 0 calls function 1; each holds one live.
    #[test]
    fn verify_temp_buffer_bound_when_call_chain_then_sums_along_it() {
        let mut caller = concat_then_store();
        caller.push(opcode::CALL);
        caller.extend_from_slice(&1u16.to_le_bytes());
        caller.extend_from_slice(&0u16.to_le_bytes());
        caller.push(opcode::POP);
        caller.push(opcode::RET_VOID);
        let caller_len = caller.len();

        let mut callee = concat_then_store();
        callee.push(opcode::RET_VOID);

        let mut bytecode = caller;
        bytecode.extend_from_slice(&callee);
        let code = CodeSection {
            functions: vec![
                FuncEntry {
                    function_id: FunctionId::new(0),
                    code_offset: 0,
                    code_length: caller_len as u32,
                    max_stack_depth: 8,
                    num_locals: 4,
                    num_params: 0,
                },
                FuncEntry {
                    function_id: FunctionId::new(1),
                    code_offset: caller_len as u32,
                    code_length: (bytecode.len() - caller_len) as u32,
                    max_stack_depth: 8,
                    num_locals: 4,
                    num_params: 0,
                },
            ],
            bytecode,
        };

        assert!(verify(&code, 1).is_err());
        assert_eq!(verify(&code, 2), Ok(()));
    }

    /// A function no entry point reaches never runs, so it must not be
    /// charged to the pool.
    #[test]
    fn verify_temp_buffer_bound_when_function_is_unreachable_then_not_charged() {
        let entry = vec![opcode::RET_VOID];
        let entry_len = entry.len();
        let mut orphan = concat_then_store();
        orphan.extend_from_slice(&concat_then_store());
        orphan.push(opcode::RET_VOID);

        let mut bytecode = entry;
        bytecode.extend_from_slice(&orphan);
        let code = CodeSection {
            functions: vec![
                FuncEntry {
                    function_id: FunctionId::new(0),
                    code_offset: 0,
                    code_length: entry_len as u32,
                    max_stack_depth: 8,
                    num_locals: 4,
                    num_params: 0,
                },
                FuncEntry {
                    function_id: FunctionId::new(1),
                    code_offset: entry_len as u32,
                    code_length: (bytecode.len() - entry_len) as u32,
                    max_stack_depth: 8,
                    num_locals: 4,
                    num_params: 0,
                },
            ],
            bytecode,
        };

        assert_eq!(verify(&code, 0), Ok(()));
    }
}
