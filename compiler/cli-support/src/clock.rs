//! Provides the clock that times PLC task execution.
//!
//! The VM measures how long each task runs (for its watchdog and execution
//! statistics) with a clock the caller passes to `run_round`. The VM crate
//! cannot read the operating system's clock itself: it is meant to build
//! without the standard library (ADR-0010). Every command line program that
//! runs the VM uses [`InstantClock`], so its watchdog sees real time.

use ironplc_vm::Clock;
use std::time::Instant;

/// A [`Clock`] that reads the operating system's monotonic clock, in
/// microseconds since the clock was created.
#[derive(Clone, Copy, Debug)]
pub struct InstantClock {
    origin: Instant,
}

impl InstantClock {
    /// A clock that reads 0 now.
    pub fn new() -> Self {
        InstantClock {
            origin: Instant::now(),
        }
    }
}

impl Default for InstantClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for InstantClock {
    fn now_us(&mut self) -> u64 {
        // A u64 holds about 584 000 years of microseconds; saturate rather
        // than truncate if that is ever exceeded.
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn now_us_when_read_twice_then_second_reading_not_earlier() {
        let mut clock = InstantClock::default();
        let first = clock.now_us();
        let second = clock.now_us();
        assert!(second >= first);
    }

    #[test]
    fn now_us_when_time_passes_then_advances_by_at_least_that_time() {
        let mut clock = InstantClock::new();
        let before = clock.now_us();
        std::thread::sleep(Duration::from_millis(2));
        let after = clock.now_us();
        assert!(after - before >= 2_000);
    }
}
