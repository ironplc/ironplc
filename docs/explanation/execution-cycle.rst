===============
Execution Cycle
===============

IronPLC runs your programs on a repeating cycle. This page explains what
happens during each cycle, how the runtime decides which tasks to run, and
what happens when things take longer than expected.

For background on how programs, tasks, resources, and configurations fit
together, see :doc:`program-organization`.

--------------------------------------
What Is the Scan Cycle?
--------------------------------------

A PLC does not run code once and exit. It runs in a continuous loop called
the **scan cycle**. Each pass through the loop is one **scheduling round**.
During each round, the runtime:

1. Reads a monotonic clock to determine the current time.
2. Checks which tasks are due to run.
3. Executes the due tasks in priority order.
4. Records timing information and advances each task's next-due time.

This loop repeats until the runtime is stopped (for example, with
:kbd:`Ctrl+C`) or until a fault occurs.

The :program:`ironplcvm` command runs this loop continuously by default.
You can limit execution to a fixed number of rounds with the ``--scans``
option — useful for testing and debugging. See the
:doc:`/reference/runtime/ironplcvm` command reference for details.

--------------------------------------
How Tasks Are Scheduled
--------------------------------------

Each scheduling round, the runtime collects all tasks that are **ready** to
run. A task's readiness depends on its type:

**Cyclic tasks**
   A cyclic task has a fixed interval (for example, ``T#100ms``). The
   runtime tracks a *next-due time* for each cyclic task. When the current
   time reaches or passes that time, the task is ready.

**Freewheeling tasks**
   A freewheeling task has no interval. It is ready on every scheduling
   round and runs as fast as the runtime allows.

Once the runtime has collected the ready tasks, it sorts them by
**priority**. Priority 0 is the highest. If two tasks have the same
priority, they run in declaration order (the order they appear in the
source code).

Within a single task, program instances execute in the order they are
declared in the :code:`RESOURCE` block.

.. tip::

   Use higher-priority (lower-numbered) tasks for time-critical logic like
   motion control, and lower-priority tasks for slower processes like
   temperature monitoring or logging.

--------------------------------------
Inside a Single Round
--------------------------------------

Here is what the runtime does in each scheduling round, step by step:

1. **Read the clock.** The runtime reads a monotonic clock to get the
   current time in microseconds since the VM started.

2. **Update system variables.** If the program uses system uptime
   variables, the runtime writes the current time into
   ``__SYSTEM_UP_TIME`` and ``__SYSTEM_UP_LTIME`` before any task
   executes.

3. **Collect and sort ready tasks.** The runtime checks every enabled task
   and collects those that are due. It then sorts the ready list by
   priority (lowest number first).

4. **Execute tasks.** For each ready task, the runtime executes all of
   the task's program instances in declaration order. Each program
   instance runs its control logic to completion before the next instance
   starts.

5. **Record timing.** After each task finishes, the runtime records how
   long execution took, increments the task's scan counter, and advances
   the task's next-due time by one interval.

6. **Repeat.** The runtime returns to step 1 for the next round.

Because all tasks run cooperatively in a single thread, a long-running
task delays every task behind it in the priority queue. The runtime does
not preempt a running task.

--------------------------------------
Overruns
--------------------------------------

An **overrun** occurs when a task's execution takes longer than its
interval. For example, if a task with a 10 ms interval takes 15 ms to
execute, the next cycle is already 5 ms overdue by the time the task
finishes.

When the runtime detects an overrun, it:

- **Skips the missed cycle.** The runtime does not try to "catch up" by
  running the task again immediately. Instead, it realigns the next-due
  time to the current time plus one interval. This prevents a cascade of
  back-to-back executions that would starve lower-priority tasks.

- **Increments an overrun counter.** The runtime tracks how many times
  each task has overrun. This counter is useful for diagnosing
  performance issues.

In short: missed cycles are dropped, not queued. The runtime always looks
forward, never backward.

.. admonition:: Practical advice

   - **Choose intervals with headroom.** If your task typically takes
     8 ms, setting an interval of 10 ms leaves very little margin. A
     20 ms interval gives you room for occasional spikes.

   - **Split heavy work across tasks.** Move non-time-critical logic
     into a separate, slower task so it does not block the fast task.

   - **Monitor overrun counts.** Frequent overruns indicate that the
     task's workload exceeds its interval. Either simplify the logic or
     increase the interval.

--------------------------------------
Watchdog Timeout
--------------------------------------

.. note::

   Watchdog enforcement is under active development.

While an overrun causes the runtime to skip a cycle and continue, a
**watchdog timeout** is a hard limit that halts the VM.

Each task can have a watchdog timeout. If a single execution of the task
exceeds the watchdog duration, the runtime triggers a **fault** and the
VM stops. This protects against runaway logic that could leave outputs in
an unsafe state.

The key difference:

- **Overrun**: the task finished, but it finished late. The runtime skips
  the missed cycle and keeps running.
- **Watchdog**: the task is still running and has exceeded a safety limit.
  The runtime stops.

A watchdog timeout of zero disables the watchdog for that task.

--------------------------------------
VM Lifecycle
--------------------------------------

The runtime follows a simple lifecycle:

1. **Load.** The VM reads the bytecode container (``.iplc`` file) and
   allocates memory for variables and program instances.

2. **Initialize.** The VM runs each program instance's initialization
   logic, setting variables to their declared initial values.

3. **Run.** The VM enters the scan cycle loop described above. It
   continues until one of:

   - The user requests a stop (:kbd:`Ctrl+C`).
   - The ``--scans`` limit is reached.
   - A fault occurs (for example, a watchdog timeout).

4. **Stop or Fault.** The VM reaches a terminal state. If it stopped
   normally, it can optionally dump all variable values to a file
   (``--dump-vars``). If it faulted, it reports the fault context
   including which task and program instance caused the fault.

--------------------------------------
See Also
--------------------------------------

- :doc:`program-organization` — how programs, tasks, resources, and
  configurations fit together
- :doc:`/reference/language/pous/task` — TASK syntax reference
- :doc:`/reference/runtime/overview` — runtime overview
- :doc:`/reference/runtime/ironplcvm` — command-line reference
