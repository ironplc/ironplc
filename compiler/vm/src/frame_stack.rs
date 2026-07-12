//! Explicit call-frame stack for iterative VM dispatch.
//!
//! The VM evaluates one opcode of the topmost frame per dispatch iteration.
//! Each PLC `CALL` pushes a [`Frame`] and each `RET` / `RET_VOID` pops one;
//! the Rust call stack is not consumed proportionally to PLC call depth.
//!
//! The frame backing slice is supplied by the embedder via [`VmBuffers`]
//! and is never resized at runtime — exactly the model already used by
//! [`OperandStack`](crate::stack::OperandStack) and
//! [`VariableTable`](crate::variable_table::VariableTable).
//!
//! [`VmBuffers`]: crate::VmBuffers

use ironplc_container::{FunctionId, VarIndex};

use crate::error::Trap;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};

/// Saved state captured on each `FB_CALL` to a user-defined function
/// block, so the dispatch loop can run the field copy-out
/// (variable slots → data region) when the corresponding frame
/// returns.
#[derive(Clone, Copy, Debug)]
pub struct FbCallReturn {
    /// Byte offset of the FB instance in the data region.
    pub instance_start: usize,
    /// First variable slot the FB instance is mapped to.
    pub var_offset: u16,
    /// Number of fields (each one 8-byte slot).
    pub num_fields: u8,
}

impl FbCallReturn {
    /// Copies each FB field's variable slot back into its location in
    /// the data region, in field order.
    ///
    /// This runs on `RET` / `RET_VOID` (and implicit fall-off-the-end)
    /// for user-FB frames so that the caller sees the post-execution
    /// values of the FB instance. The byte stride (`size_of::<Slot>()`)
    /// and little-endian encoding stay encapsulated here so the
    /// dispatch loop never has to spell out the slot layout.
    pub fn copy_out(&self, variables: &VariableTable, data_region: &mut [u8]) -> Result<(), Trap> {
        const STRIDE: usize = core::mem::size_of::<Slot>();
        for i in 0..self.num_fields as usize {
            let offset = self.instance_start + i * STRIDE;
            if offset + STRIDE > data_region.len() {
                return Err(Trap::DataRegionOutOfBounds(offset as u32));
            }
            let val = variables.load(VarIndex::new(self.var_offset + i as u16))?;
            data_region[offset..offset + STRIDE].copy_from_slice(&val.as_i64().to_le_bytes());
        }
        Ok(())
    }
}

/// One PLC call frame.
///
/// `Copy` so the backing slice can use any contiguous storage, including
/// fixed-size arrays on `no_std` targets that do not have `Vec`.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Function whose bytecode this frame is executing.
    pub function_id: FunctionId,
    /// Byte offset of the next opcode to execute within the function's
    /// bytecode.
    pub pc: usize,
    /// Variable scope (shared-globals window + this frame's locals window).
    pub scope: VariableScope,
    /// Snapshot of the temp-buffer allocator's `next` counter at the
    /// moment this frame was pushed. Restored on return so the caller's
    /// previously-handed-out temp slot indices remain valid.
    pub temp_alloc_mark: u16,
    /// If `Some`, this frame was pushed by an `FB_CALL` on a user-defined
    /// function block. On return, the dispatch loop runs the field
    /// copy-out using the saved instance pointer.
    pub fb_return: Option<FbCallReturn>,
}

/// Bounded-capacity call-frame stack backed by a borrowed slice.
///
/// Mirrors [`OperandStack`](crate::stack::OperandStack) /
/// [`VariableTable`](crate::variable_table::VariableTable): allocates
/// nothing, traps on push past capacity via [`Trap::CallStackOverflow`].
pub struct FrameStack<'a> {
    slots: &'a mut [Frame],
    len: usize,
}

impl<'a> FrameStack<'a> {
    /// Build an empty frame stack over the given backing slice.
    pub fn new(backing: &'a mut [Frame]) -> Self {
        FrameStack {
            slots: backing,
            len: 0,
        }
    }

    /// Rebuild a frame stack over `backing` with `len` frames already live.
    ///
    /// Used by the re-entrant debug driver to resume a paused instance: the
    /// frame contents survive in the embedder's backing slice across a
    /// pause, and this restores the logical length so the dispatch loop
    /// continues from the preserved top frame.
    pub fn resume(backing: &'a mut [Frame], len: usize) -> Self {
        debug_assert!(len <= backing.len(), "resume len exceeds frame capacity");
        FrameStack {
            slots: backing,
            len,
        }
    }

    /// Number of frames currently on the stack.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the stack contains zero frames.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Capacity of the backing slice (the maximum allowed depth).
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Push a new frame.
    ///
    /// Returns [`Trap::CallStackOverflow`] when the backing slice is full.
    pub fn push(&mut self, frame: Frame) -> Result<(), Trap> {
        if self.len >= self.slots.len() {
            return Err(Trap::CallStackOverflow);
        }
        self.slots[self.len] = frame;
        self.len += 1;
        Ok(())
    }

    /// Pop the topmost frame, returning it. `None` if the stack is empty.
    pub fn pop(&mut self) -> Option<Frame> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.slots[self.len])
    }

    /// Peek at the topmost frame.
    pub fn top(&self) -> Option<&Frame> {
        if self.len == 0 {
            None
        } else {
            Some(&self.slots[self.len - 1])
        }
    }

    /// Mutable view of the topmost frame.
    pub fn top_mut(&mut self) -> Option<&mut Frame> {
        if self.len == 0 {
            None
        } else {
            Some(&mut self.slots[self.len - 1])
        }
    }

    /// Empty the stack without touching the backing storage.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(fn_id: u16, pc: usize) -> Frame {
        Frame {
            function_id: FunctionId::new(fn_id),
            pc,
            scope: VariableScope {
                shared_globals_size: 0,
                instance_offset: 0,
                instance_count: 0,
            },
            temp_alloc_mark: 0,
            fb_return: None,
        }
    }

    #[test]
    fn frame_stack_new_when_created_then_empty() {
        let mut buf = [frame(0, 0); 4];
        let stack = FrameStack::new(&mut buf);
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        assert_eq!(stack.capacity(), 4);
        assert!(stack.top().is_none());
    }

    #[test]
    fn frame_stack_push_pop_when_used_then_lifo() {
        let mut buf = [frame(0, 0); 4];
        let mut stack = FrameStack::new(&mut buf);

        stack.push(frame(1, 10)).unwrap();
        stack.push(frame(2, 20)).unwrap();
        stack.push(frame(3, 30)).unwrap();
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.top().unwrap().function_id, FunctionId::new(3));

        let p = stack.pop().unwrap();
        assert_eq!(p.function_id, FunctionId::new(3));
        assert_eq!(p.pc, 30);
        assert_eq!(stack.top().unwrap().function_id, FunctionId::new(2));

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());
        assert!(stack.pop().is_none());
    }

    #[test]
    fn frame_stack_push_when_at_capacity_then_traps_call_stack_overflow() {
        let mut buf = [frame(0, 0); 2];
        let mut stack = FrameStack::new(&mut buf);

        stack.push(frame(1, 0)).unwrap();
        stack.push(frame(2, 0)).unwrap();
        assert_eq!(stack.push(frame(3, 0)), Err(Trap::CallStackOverflow));
    }

    #[test]
    fn frame_stack_top_mut_when_mutated_then_persists() {
        let mut buf = [frame(0, 0); 2];
        let mut stack = FrameStack::new(&mut buf);
        stack.push(frame(7, 0)).unwrap();
        stack.top_mut().unwrap().pc = 42;
        assert_eq!(stack.top().unwrap().pc, 42);
    }
}
