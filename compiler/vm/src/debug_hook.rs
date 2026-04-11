//! Debug hook trait for instruction-level instrumentation.
//!
//! The VM's instruction dispatch loop calls
//! [`DebugHook::before_instruction`] before executing each opcode. This
//! provides a single, well-defined extension point for debuggers, profilers,
//! tracers, and breakpoint handlers — without forcing every consumer of the
//! VM to pay for the feature.
//!
//! ## Zero-cost when unused
//!
//! [`NoopDebugHook`] is a zero-sized type whose
//! [`before_instruction`](DebugHook::before_instruction) is `#[inline(always)]`
//! and has an empty body. When the VM is monomorphized over `NoopDebugHook`
//! the optimizer eliminates the call entirely, so VMs that do not need
//! instruction-level callbacks pay no runtime cost.
//!
//! Custom hooks (e.g. a breakpoint table or tracer) implement [`DebugHook`]
//! on their own type. The VM is generic over the hook type, so each hook
//! gets its own monomorphized dispatch loop.

/// A trait invoked by the VM before executing each instruction.
///
/// Implementations may inspect or react to the upcoming instruction —
/// for example, by checking a breakpoint table, recording a trace, or
/// pausing for a debugger. Implementations must not mutate the VM's
/// state through side channels; they only see the program counter and
/// opcode byte.
pub trait DebugHook {
    /// Called immediately before the instruction at `pc` (with opcode
    /// byte `op`) is executed.
    ///
    /// `pc` is the byte offset of the opcode within the currently
    /// executing function's bytecode (i.e. the position of `op` itself,
    /// not the position of the next byte to be read).
    fn before_instruction(&mut self, pc: usize, op: u8);
}

/// A no-op [`DebugHook`] used by default. Zero-sized; the empty
/// `before_instruction` is always inlined and compiles to nothing.
#[derive(Default, Clone, Copy, Debug)]
pub struct NoopDebugHook;

impl DebugHook for NoopDebugHook {
    #[inline(always)]
    fn before_instruction(&mut self, _pc: usize, _op: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn noop_debug_hook_when_called_then_does_nothing() {
        let mut hook = NoopDebugHook;
        hook.before_instruction(0, 1);
        hook.before_instruction(usize::MAX, u8::MAX);
        // No assertion needed: hook has no observable state.
    }

    #[test]
    fn custom_debug_hook_when_called_then_records_each_instruction() {
        struct RecordingHook {
            events: Vec<(usize, u8)>,
        }
        impl DebugHook for RecordingHook {
            fn before_instruction(&mut self, pc: usize, op: u8) {
                self.events.push((pc, op));
            }
        }
        let mut hook = RecordingHook { events: Vec::new() };
        hook.before_instruction(0, 0x10);
        hook.before_instruction(2, 0x11);
        assert_eq!(hook.events, vec![(0, 0x10), (2, 0x11)]);
    }
}
