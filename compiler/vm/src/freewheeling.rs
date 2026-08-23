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
//! loads it: [`Vm::load`](crate::Vm::load) copies `task_type` and `interval_us`
//! into each `TaskState`, so the scheduler, `next_due_us()`, and every
//! cycle-derived timestamp follow from the rewrite with no special case
//! anywhere else.
//!
//! Callers must report the assumption they ran under — a trace whose timestamps
//! depend on an assumed rate is not self-describing without it.

use ironplc_container::{Container, TaskType};

/// The assumed cycle time for a freewheeling task, in microseconds, when the
/// caller does not supply one.
///
/// 100 ms is a plausible scan time for a small program on modest hardware. It
/// is deliberately not a measured or derived value: no such value exists for a
/// task whose whole definition is "as fast as possible".
pub const DEFAULT_FREEWHEELING_INTERVAL_US: u64 = 100_000;

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
/// running at `interval_us`, and returns how many tasks were rewritten.
///
/// Returns 0 — leaving the container untouched — when it declares no
/// freewheeling task, or when `interval_us` is 0. A zero interval would make
/// the rewritten task permanently overdue, inflating `overrun_count` on every
/// round; codegen avoids the same trap by compiling `INTERVAL := T#0s` to a
/// freewheeling task in the first place.
pub fn assume_freewheeling_interval(container: &mut Container, interval_us: u64) -> usize {
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

/// Bit 0 of a `TaskEntry`'s flags marks the task enabled, matching the test
/// `Vm::load` applies when it populates `TaskState::enabled`.
fn is_enabled(flags: u8) -> bool {
    (flags & 0x01) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::{
        ContainerBuilder, FunctionId, TaskEntry, TaskId, TaskTable, VarIndex,
    };

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

        assert_eq!(assume_freewheeling_interval(&mut container, 100_000), 1);

        let rewritten = &container.task_table.tasks[0];
        assert_eq!(rewritten.task_type, TaskType::Cyclic);
        assert_eq!(rewritten.interval_us, 100_000);
    }

    #[test]
    fn assume_freewheeling_interval_when_cyclic_then_leaves_declared_interval() {
        let mut container = container_with(vec![task(TaskType::Cyclic, 10_000, true)]);

        assert_eq!(assume_freewheeling_interval(&mut container, 100_000), 0);

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

        assert_eq!(assume_freewheeling_interval(&mut container, 100_000), 1);

        assert_eq!(container.task_table.tasks[0].interval_us, 10_000);
        assert_eq!(container.task_table.tasks[1].interval_us, 100_000);
    }

    #[test]
    fn assume_freewheeling_interval_when_zero_interval_then_leaves_freewheeling() {
        let mut container = container_with(vec![task(TaskType::Freewheeling, 0, true)]);

        assert_eq!(assume_freewheeling_interval(&mut container, 0), 0);

        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
    }

    #[test]
    fn assume_freewheeling_interval_when_disabled_then_leaves_freewheeling() {
        let mut container = container_with(vec![task(TaskType::Freewheeling, 0, false)]);

        assert_eq!(assume_freewheeling_interval(&mut container, 100_000), 0);

        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
    }
}
