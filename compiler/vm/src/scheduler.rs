use ironplc_container::TaskType;

/// Per-task runtime state tracked by the scheduler.
#[derive(Clone, Debug, Default)]
pub struct TaskState {
    pub task_id: u16,
    pub priority: u16,
    pub task_type: TaskType,
    pub interval_us: u64,
    pub watchdog_us: u64,
    pub enabled: bool,
    pub next_due_us: u64,
    pub scan_count: u64,
    pub last_execute_us: u64,
    pub max_execute_us: u64,
    pub overrun_count: u64,
}

/// Per-program-instance runtime state.
#[derive(Clone, Debug, Default)]
pub struct ProgramInstanceState {
    pub instance_id: u16,
    pub task_id: u16,
    pub entry_function_id: u16,
    pub var_table_offset: u16,
    pub var_table_count: u16,
}

/// Cooperative task scheduler that determines which tasks to execute each round.
///
/// The scheduler is time-agnostic: callers pass the current time as a `u64`
/// microsecond value. This makes the scheduler fully testable without mocking clocks.
pub struct TaskScheduler<'a> {
    pub task_states: &'a mut [TaskState],
}

impl<'a> TaskScheduler<'a> {
    /// Creates a scheduler from a caller-provided task state slice.
    pub fn new(task_states: &'a mut [TaskState]) -> Self {
        TaskScheduler { task_states }
    }

    /// Returns indices into `task_states` for tasks that are ready to execute,
    /// sorted by priority (ascending) then task_id (ascending).
    ///
    /// Writes into the caller-provided buffer and returns a sub-slice of ready indices.
    pub fn collect_ready_tasks<'b>(
        &self,
        current_time_us: u64,
        buf: &'b mut [usize],
    ) -> &'b [usize] {
        let mut count = 0;
        for (i, t) in self.task_states.iter().enumerate() {
            if !t.enabled {
                continue;
            }
            let ready = match t.task_type {
                TaskType::Freewheeling => true,
                TaskType::Cyclic => current_time_us >= t.next_due_us,
                TaskType::Event => false, // Phase 3
            };
            if ready && count < buf.len() {
                buf[count] = i;
                count += 1;
            }
        }

        let ready = &mut buf[..count];

        // Sort by priority (ascending) then task_id (ascending).
        // Use a simple insertion sort since the number of tasks is small.
        for i in 1..ready.len() {
            let mut j = i;
            while j > 0 {
                let a = ready[j - 1];
                let b = ready[j];
                let ta = &self.task_states[a];
                let tb = &self.task_states[b];
                let cmp = ta
                    .priority
                    .cmp(&tb.priority)
                    .then(ta.task_id.cmp(&tb.task_id));
                if cmp == core::cmp::Ordering::Greater {
                    ready.swap(j - 1, j);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        &buf[..count]
    }

    /// Records that a task executed, updating timing and overrun tracking.
    pub fn record_execution(&mut self, task_index: usize, elapsed_us: u64, current_time_us: u64) {
        let task = &mut self.task_states[task_index];
        task.scan_count += 1;
        task.last_execute_us = elapsed_us;
        if elapsed_us > task.max_execute_us {
            task.max_execute_us = elapsed_us;
        }

        if task.task_type == TaskType::Cyclic {
            task.next_due_us += task.interval_us;
            if task.next_due_us <= current_time_us {
                task.overrun_count += 1;
                task.next_due_us = current_time_us + task.interval_us;
            }
        }
    }

    /// Returns the earliest `next_due_us` across all enabled cyclic tasks,
    /// or `None` if no cyclic tasks exist.
    // Used in tests; will be called from the CLI run-loop once cyclic sleep is added.
    #[allow(dead_code)]
    pub fn next_due_us(&self) -> Option<u64> {
        self.task_states
            .iter()
            .filter(|t| t.enabled && t.task_type == TaskType::Cyclic)
            .map(|t| t.next_due_us)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::{ProgramInstanceEntry, TaskEntry, TaskTable};

    fn freewheeling_task_table() -> TaskTable {
        TaskTable {
            shared_globals_size: 0,
            tasks: vec![TaskEntry {
                task_id: 0,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x01,
                interval_us: 0,
                single_var_index: 0xFFFF,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: 0,
                task_id: 0,
                entry_function_id: 0,
                var_table_offset: 0,
                var_table_count: 2,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                reserved: 0,
            }],
        }
    }

    fn two_cyclic_tasks_table() -> TaskTable {
        TaskTable {
            shared_globals_size: 2,
            tasks: vec![
                TaskEntry {
                    task_id: 0,
                    priority: 5,
                    task_type: TaskType::Cyclic,
                    flags: 0x01,
                    interval_us: 100_000,
                    single_var_index: 0xFFFF,
                    watchdog_us: 0,
                    input_image_offset: 0,
                    output_image_offset: 0,
                    reserved: [0; 4],
                },
                TaskEntry {
                    task_id: 1,
                    priority: 0,
                    task_type: TaskType::Cyclic,
                    flags: 0x01,
                    interval_us: 10_000,
                    single_var_index: 0xFFFF,
                    watchdog_us: 0,
                    input_image_offset: 0,
                    output_image_offset: 0,
                    reserved: [0; 4],
                },
            ],
            programs: vec![
                ProgramInstanceEntry {
                    instance_id: 0,
                    task_id: 0,
                    entry_function_id: 0,
                    var_table_offset: 2,
                    var_table_count: 3,
                    fb_instance_offset: 0,
                    fb_instance_count: 0,
                    reserved: 0,
                },
                ProgramInstanceEntry {
                    instance_id: 1,
                    task_id: 1,
                    entry_function_id: 1,
                    var_table_offset: 5,
                    var_table_count: 3,
                    fb_instance_offset: 0,
                    fb_instance_count: 0,
                    reserved: 0,
                },
            ],
        }
    }

    /// Populates task_states from a TaskTable.
    fn populate_task_states(table: &TaskTable, task_states: &mut [TaskState]) {
        for (i, t) in table.tasks.iter().enumerate() {
            task_states[i] = TaskState {
                task_id: t.task_id,
                priority: t.priority,
                task_type: t.task_type,
                interval_us: t.interval_us,
                watchdog_us: t.watchdog_us,
                enabled: (t.flags & 0x01) != 0,
                next_due_us: 0,
                scan_count: 0,
                last_execute_us: 0,
                max_execute_us: 0,
                overrun_count: 0,
            };
        }
    }

    #[test]
    fn from_task_table_when_freewheeling_then_one_task_one_program() {
        let table = freewheeling_task_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let sched = TaskScheduler::new(&mut task_states);

        assert_eq!(sched.task_states.len(), 1);
        assert_eq!(sched.task_states[0].task_type, TaskType::Freewheeling);
        assert!(sched.task_states[0].enabled);
    }

    #[test]
    fn collect_ready_when_freewheeling_then_always_ready() {
        let table = freewheeling_task_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let sched = TaskScheduler::new(&mut task_states);

        let mut buf = [0usize; 8];
        let ready = sched.collect_ready_tasks(0, &mut buf);
        assert_eq!(ready, &[0]);
    }

    #[test]
    fn collect_ready_when_cyclic_at_time_zero_then_all_due() {
        let table = two_cyclic_tasks_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let sched = TaskScheduler::new(&mut task_states);

        let mut buf = [0usize; 8];
        let ready = sched.collect_ready_tasks(0, &mut buf);
        assert_eq!(ready, &[1, 0]);
    }

    #[test]
    fn collect_ready_when_cyclic_not_due_then_empty() {
        let table = two_cyclic_tasks_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let mut sched = TaskScheduler::new(&mut task_states);

        sched.record_execution(0, 100, 0);
        sched.record_execution(1, 100, 0);

        let mut buf = [0usize; 8];
        let ready = sched.collect_ready_tasks(5_000, &mut buf);
        assert!(ready.is_empty());
    }

    #[test]
    fn collect_ready_when_fast_task_due_slow_not_then_only_fast() {
        let table = two_cyclic_tasks_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let mut sched = TaskScheduler::new(&mut task_states);

        sched.record_execution(0, 100, 0);
        sched.record_execution(1, 100, 0);

        let mut buf = [0usize; 8];
        let ready = sched.collect_ready_tasks(10_000, &mut buf);
        assert_eq!(ready, &[1]);
    }

    #[test]
    fn collect_ready_when_task_disabled_then_skipped() {
        let mut table = freewheeling_task_table();
        table.tasks[0].flags = 0x00;
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let sched = TaskScheduler::new(&mut task_states);

        let mut buf = [0usize; 8];
        let ready = sched.collect_ready_tasks(0, &mut buf);
        assert!(ready.is_empty());
    }

    #[test]
    fn record_execution_when_cyclic_overrun_then_realigns() {
        let table = two_cyclic_tasks_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let mut sched = TaskScheduler::new(&mut task_states);

        sched.record_execution(1, 100, 0);
        assert_eq!(sched.task_states[1].next_due_us, 10_000);
        sched.record_execution(1, 100, 25_000);
        assert_eq!(sched.task_states[1].next_due_us, 35_000);
        assert_eq!(sched.task_states[1].overrun_count, 1);
    }

    #[test]
    fn next_due_when_cyclic_tasks_then_returns_earliest() {
        let table = two_cyclic_tasks_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let mut sched = TaskScheduler::new(&mut task_states);

        sched.record_execution(0, 100, 0);
        sched.record_execution(1, 100, 0);
        assert_eq!(sched.next_due_us(), Some(10_000));
    }

    #[test]
    fn next_due_when_only_freewheeling_then_none() {
        let table = freewheeling_task_table();
        let mut task_states = vec![TaskState::default(); table.tasks.len()];
        populate_task_states(&table, &mut task_states);
        let sched = TaskScheduler::new(&mut task_states);

        assert_eq!(sched.next_due_us(), None);
    }
}
