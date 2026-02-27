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
On each scheduling round, the runtime checks which tasks are due to run
based on elapsed time and executes them in priority order.

By default, :program:`ironplcvm` runs continuously, executing scheduling
rounds until interrupted with :kbd:`Ctrl+C`. You can also limit execution
to a fixed number of rounds using the ``--scans`` option.

Variable Inspection
===================

The runtime can dump all variable values to a file after execution completes.
This is useful for verifying program behavior and debugging. Use the
``--dump-vars`` option to specify an output file.

See the :doc:`ironplcvm command reference </reference/runtime/ironplcvm>` for
complete usage details.
