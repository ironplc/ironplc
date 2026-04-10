========
Overview
========

The IronPLC runtime is a virtual machine that executes compiled IEC 61131-3
programs. It reads bytecode container (``.iplc``) files produced by the
:program:`ironplcc` compiler and runs them according to the task configuration
defined in the source program.

Execution Model
===============

The runtime follows the IEC 61131-3 execution model. A compiled program
contains one or more tasks, each with an associated interval and priority.
The runtime runs a continuous loop called the **scan cycle**. Each pass
through the loop is one scheduling round.

On each round, the runtime reads a monotonic clock, collects all tasks
whose interval has elapsed, sorts them by priority (0 is highest), and
executes them in order. Within a task, program instances run in the order
they are declared in the source code. All tasks execute cooperatively in
a single thread — the runtime does not preempt a running task.

If a task takes longer than its interval, the runtime **skips the missed
cycle** and realigns the next-due time forward. This prevents a cascade
of back-to-back executions. The runtime tracks an overrun counter for
each task so you can detect when this happens.

Each task can also have a **watchdog timeout**. If a single execution
exceeds the watchdog duration, the runtime triggers a fault and the VM
stops. Unlike an overrun (which skips a cycle and continues), a watchdog
timeout halts execution to protect against runaway logic.

By default, :program:`ironplcvm` runs continuously, executing scheduling
rounds until interrupted with :kbd:`Ctrl+C`. You can also limit execution
to a fixed number of rounds using the ``--scans`` option.

For a detailed explanation of the scan cycle, overruns, watchdog behavior,
and the VM lifecycle, see :doc:`/explanation/execution-cycle`.

Variable Inspection
===================

The runtime can dump all variable values to a file after execution completes.
This is useful for verifying program behavior and debugging. Use the
``--dump-vars`` option to specify an output file.

See the :doc:`ironplcvm command reference </reference/runtime/ironplcvm>` for
complete usage details.
