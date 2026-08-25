//! The assumed cycle time for freewheeling tasks under simulated time.
//!
//! A freewheeling task declares no `INTERVAL`: it runs as fast as the hardware
//! allows, so its cycle rate is a property of the machine rather than of the
//! program. Under a real clock that is fine — `ironplcvm run` measures elapsed
//! time and never needs the rate. Under *simulated* time (the `run` MCP tool,
//! the debugger) there is no clock to measure, so nothing advances program time
//! and a run driven by `next_due_us()` stops after a single cycle.
//!
//! The resolution is to make the rate an input with a documented default rather
//! than a number the runtime invents silently. A freewheeling task plus an
//! assumed cycle time is exactly a cyclic task at that interval, so the
//! assumption is applied by rewriting the container's task table before the VM
//! loads it: [`Vm::load`](crate::Vm::load) copies the task type and interval
//! into each `TaskState`, so the scheduler, `next_due_us()`, and every
//! cycle-derived timestamp follow from the rewrite with no special case
//! anywhere else.
//!
//! Callers must report the assumption they ran under — a trace whose timestamps
//! depend on an assumed rate is not self-describing without it.

use core::fmt;
use core::time::Duration;

use ironplc_container::{Container, TaskType};

/// The cycle time assumed for a freewheeling task when the caller supplies
/// none.
///
/// 100 ms is a plausible scan time for a small program on modest hardware. It
/// is deliberately not a measured or derived value: no such value exists for a
/// task whose whole definition is "as fast as possible".
///
/// The two callers deliberately differ on whether to reach for it, and the
/// reason is recorded here rather than in either so that neither has to
/// restate the other's:
///
/// - The `run` MCP tool never applies it. Its caller is an agent, and a trace
///   built on a rate the agent never chose is a trap, so the run is rejected
///   with a diagnostic naming what to supply (REQ-TOL-mcp-049).
/// - The debugger applies it. Its caller is a person mid-session, and a debug
///   session that refuses to start is a poor answer to an omitted or mistyped
///   launch setting. The session says which rate it assumed instead.
pub const DEFAULT_FREEWHEELING_INTERVAL: Duration = Duration::from_millis(100);

/// The longest cycle time a caller may ask for.
///
/// An interval longer than any plausible run produces an empty trace, which is
/// a confusing way to learn the value was a mistake.
pub const MAX_FREEWHEELING_INTERVAL: Duration = Duration::from_secs(3_600);

/// Why a caller-supplied cycle time is not one.
///
/// Carrying the reason — rather than an unexplained `None` — lets each caller
/// state the bound it actually enforces instead of restating one from memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntervalError {
    /// Not a finite duration greater than zero.
    NotPositive,
    /// Longer than [`MAX_FREEWHEELING_INTERVAL`].
    TooLong,
}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntervalError::NotPositive => {
                write!(f, "must be a finite number of milliseconds greater than 0")
            }
            IntervalError::TooLong => write!(
                f,
                "must be at most {} ms (one hour)",
                MAX_FREEWHEELING_INTERVAL.as_millis()
            ),
        }
    }
}

/// Converts a caller-supplied cycle time in milliseconds to a [`Duration`].
///
/// Zero or negative would leave the rewritten task permanently overdue, and a
/// non-finite value has no duration at all. Conversion is at microsecond
/// granularity because a freewheeling scan is often well under a millisecond,
/// which is where the interesting values are.
///
/// Both the `run` MCP tool and the debugger take the cycle time from their
/// caller, so the rule for what counts as one lives here rather than in either.
pub fn interval_from_ms(ms: f64) -> Result<Duration, IntervalError> {
    if !ms.is_finite() || ms <= 0.0 {
        return Err(IntervalError::NotPositive);
    }
    let interval = Duration::from_micros((ms * 1_000.0).round() as u64);
    if interval > MAX_FREEWHEELING_INTERVAL {
        return Err(IntervalError::TooLong);
    }
    Ok(interval)
}

/// Returns true when `container` has at least one enabled freewheeling task,
/// and therefore needs an assumed cycle time to run under simulated time.
pub fn has_freewheeling_task(container: &Container) -> bool {
    container
        .task_table
        .tasks
        .iter()
        .any(|t| t.task_type == TaskType::Freewheeling && is_enabled(t.flags))
}

/// Rewrites every enabled freewheeling task in `container` into a cyclic task
/// running at `interval`, and returns how many tasks were rewritten.
///
/// Returns 0 — leaving the container untouched — when it declares no
/// freewheeling task, or when `interval` rounds to zero microseconds. A zero
/// interval would make the rewritten task permanently overdue, inflating
/// `overrun_count` on every round; codegen avoids the same trap by compiling
/// `INTERVAL := T#0s` to a freewheeling task in the first place.
pub fn assume_freewheeling_interval(container: &mut Container, interval: Duration) -> usize {
    let interval_us = interval_us(interval);
    if interval_us == 0 {
        return 0;
    }

    let mut rewritten = 0;
    for task in container.task_table.tasks.iter_mut() {
        if task.task_type == TaskType::Freewheeling && is_enabled(task.flags) {
            task.task_type = TaskType::Cyclic;
            task.interval_us = interval_us;
            rewritten += 1;
        }
    }
    rewritten
}

/// The cycle time the container's first enabled task runs at, or `None` when
/// it has no enabled task.
///
/// This is the interval a simulated run advances by, once
/// [`assume_freewheeling_interval`] has given the freewheeling tasks one. The
/// VM is single-task today; a container with several would need this reported
/// per task.
pub fn first_task_interval(container: &Container) -> Option<Duration> {
    container
        .task_table
        .tasks
        .iter()
        .find(|t| is_enabled(t.flags))
        .map(|t| Duration::from_micros(t.interval_us))
}

/// A `Duration` as the whole microseconds the container format and the VM
/// clock both count in, saturating rather than wrapping.
pub fn interval_us(interval: Duration) -> u64 {
    u64::try_from(interval.as_micros()).unwrap_or(u64::MAX)
}

/// Bit 0 of a `TaskEntry`'s flags marks the task enabled, matching the test
/// `Vm::load` applies when it populates `TaskState::enabled`.
fn is_enabled(flags: u8) -> bool {
    (flags & 0x01) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::{ContainerBuilder, FunctionId, TaskEntry, TaskId, TaskTable, VarIndex};

    /// A task entry in the shape `ContainerBuilder` synthesizes, so the tests
    /// exercise the same flags the real path produces.
    fn task(task_type: TaskType, interval_us: u64, enabled: bool) -> TaskEntry {
        TaskEntry {
            task_id: TaskId::DEFAULT,
            priority: 0,
            task_type,
            flags: if enabled { 0x01 } else { 0x00 },
            interval_us,
            single_var_index: VarIndex::NO_SINGLE_VAR,
            watchdog_us: 0,
            input_image_offset: 0,
            output_image_offset: 0,
            reserved: [0; 4],
        }
    }

    fn container_with(tasks: Vec<TaskEntry>) -> Container {
        let mut container = ContainerBuilder::new()
            .entry_function_id(FunctionId::new(0))
            .build();
        container.task_table = TaskTable {
            tasks,
            ..container.task_table
        };
        container
    }

    #[test]
    fn interval_from_ms_when_whole_ms_then_converts() {
        assert_eq!(interval_from_ms(100.0), Ok(Duration::from_millis(100)));
    }

    #[test]
    fn interval_from_ms_when_sub_millisecond_then_keeps_microseconds() {
        assert_eq!(interval_from_ms(0.25), Ok(Duration::from_micros(250)));
    }

    #[test]
    fn interval_from_ms_when_not_positive_then_error_says_so() {
        for ms in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(interval_from_ms(ms), Err(IntervalError::NotPositive));
        }
    }

    #[test]
    fn interval_from_ms_when_longer_than_maximum_then_error_says_so() {
        let over = MAX_FREEWHEELING_INTERVAL.as_millis() as f64 + 1.0;
        assert_eq!(interval_from_ms(over), Err(IntervalError::TooLong));
    }

    #[test]
    fn interval_from_ms_when_at_maximum_then_converts() {
        let at = MAX_FREEWHEELING_INTERVAL.as_millis() as f64;
        assert_eq!(interval_from_ms(at), Ok(MAX_FREEWHEELING_INTERVAL));
    }

    #[test]
    fn interval_error_when_displayed_then_states_the_bound_it_enforces() {
        assert_eq!(
            IntervalError::TooLong.to_string(),
            "must be at most 3600000 ms (one hour)"
        );
        assert!(IntervalError::NotPositive
            .to_string()
            .contains("greater than 0"));
    }

    #[test]
    fn first_task_interval_when_enabled_task_then_returns_its_interval() {
        let container = container_with(vec![task(TaskType::Cyclic, 10_000, true)]);
        assert_eq!(
            first_task_interval(&container),
            Some(Duration::from_millis(10))
        );
    }

    #[test]
    fn first_task_interval_when_no_enabled_task_then_none() {
        let container = container_with(vec![task(TaskType::Cyclic, 10_000, false)]);
        assert_eq!(first_task_interval(&container), None);
    }

    #[test]
    fn has_freewheeling_task_when_freewheeling_then_true() {
        let container = container_with(vec![task(TaskType::Freewheeling, 0, true)]);
        assert!(has_freewheeling_task(&container));
    }

    #[test]
    fn has_freewheeling_task_when_cyclic_then_false() {
        let container = container_with(vec![task(TaskType::Cyclic, 10_000, true)]);
        assert!(!has_freewheeling_task(&container));
    }

    #[test]
    fn has_freewheeling_task_when_freewheeling_disabled_then_false() {
        let container = container_with(vec![task(TaskType::Freewheeling, 0, false)]);
        assert!(!has_freewheeling_task(&container));
    }

    #[test]
    fn assume_freewheeling_interval_when_freewheeling_then_rewrites_to_cyclic() {
        let mut container = container_with(vec![task(TaskType::Freewheeling, 0, true)]);

        assert_eq!(
            assume_freewheeling_interval(&mut container, Duration::from_millis(100)),
            1
        );

        let rewritten = &container.task_table.tasks[0];
        assert_eq!(rewritten.task_type, TaskType::Cyclic);
        assert_eq!(rewritten.interval_us, 100_000);
    }

    #[test]
    fn assume_freewheeling_interval_when_cyclic_then_leaves_declared_interval() {
        let mut container = container_with(vec![task(TaskType::Cyclic, 10_000, true)]);

        assert_eq!(
            assume_freewheeling_interval(&mut container, Duration::from_millis(100)),
            0
        );

        let untouched = &container.task_table.tasks[0];
        assert_eq!(untouched.task_type, TaskType::Cyclic);
        assert_eq!(untouched.interval_us, 10_000);
    }

    #[test]
    fn assume_freewheeling_interval_when_mixed_then_rewrites_only_freewheeling() {
        let mut container = container_with(vec![
            task(TaskType::Cyclic, 10_000, true),
            task(TaskType::Freewheeling, 0, true),
        ]);

        assert_eq!(
            assume_freewheeling_interval(&mut container, Duration::from_millis(100)),
            1
        );

        assert_eq!(container.task_table.tasks[0].interval_us, 10_000);
        assert_eq!(container.task_table.tasks[1].interval_us, 100_000);
    }

    #[test]
    fn assume_freewheeling_interval_when_zero_then_leaves_freewheeling() {
        let mut container = container_with(vec![task(TaskType::Freewheeling, 0, true)]);

        assert_eq!(
            assume_freewheeling_interval(&mut container, Duration::ZERO),
            0
        );

        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
    }

    #[test]
    fn assume_freewheeling_interval_when_disabled_then_leaves_freewheeling() {
        let mut container = container_with(vec![task(TaskType::Freewheeling, 0, false)]);

        assert_eq!(
            assume_freewheeling_interval(&mut container, Duration::from_millis(100)),
            0
        );

        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
    }
}
