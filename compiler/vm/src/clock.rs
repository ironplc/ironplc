//! The clock that times task execution.

/// A monotonic clock, read in microseconds.
///
/// [`VmRunning::run_round`](crate::VmRunning::run_round) reads it before and
/// after each task to measure how long the task executed. That measurement
/// is what the watchdog compares against the task's `watchdog_us` and what
/// the scheduler records as the task's execution time. It is not the PLC's
/// uptime: the caller passes that to `run_round` separately as `uptime_us`,
/// and may simulate it.
///
/// The embedder supplies the clock so that the VM needs no operating-system
/// time source (ADR-0010). It is an argument to `run_round`, in the same way
/// as the hook of `run_round_debug`, rather than state the VM keeps: storing
/// it would put a type parameter or a trait object on `VmRunning`. The crate
/// provides no implementation outside `test_support`, because a clock that
/// never advances disables the watchdog, so every embedder must choose its
/// clock explicitly. The command line programs share
/// `ironplc_cli_support::clock::InstantClock`.
///
/// Readings should not decrease. If one does, the measured time saturates
/// at zero instead of wrapping.
pub trait Clock {
    /// The current reading, in microseconds from an arbitrary fixed origin.
    fn now_us(&mut self) -> u64;
}
