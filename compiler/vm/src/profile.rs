//! Instruction-level profiling for the VM.
//!
//! When the `profiling` feature is enabled, the VM tracks how many times
//! each primary opcode is executed. This guides optimization by revealing
//! which instructions dominate a workload.

use ironplc_container::Opcode;

/// Per-opcode execution counters.
///
/// Indexed directly by opcode value — a 256-element array covers every
/// possible `Opcode` (u8) with no bounds checks or hashing. The array
/// is 2 KiB and fits comfortably in L1 cache.
#[derive(Clone)]
pub struct InstructionProfile {
    counts: [u64; 256],
}

impl InstructionProfile {
    /// Creates a new profile with all counters at zero.
    pub fn new() -> Self {
        Self { counts: [0; 256] }
    }

    /// Increments the counter for the given opcode.
    #[inline(always)]
    pub fn record(&mut self, op: Opcode) {
        self.counts[op as usize] += 1;
    }

    /// Returns the execution count for a specific opcode.
    pub fn count(&self, op: Opcode) -> u64 {
        self.counts[op as usize]
    }

    /// Returns a reference to the full 256-element counts array.
    pub fn counts(&self) -> &[u64; 256] {
        &self.counts
    }

    /// Returns the total number of instructions executed.
    pub fn total(&self) -> u64 {
        self.counts.iter().sum()
    }

    /// Resets all counters to zero.
    pub fn reset(&mut self) {
        self.counts = [0; 256];
    }
}

impl Default for InstructionProfile {
    fn default() -> Self {
        Self::new()
    }
}
