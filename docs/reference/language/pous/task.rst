====
TASK
====

A task defines the scheduling of program execution within a resource.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.7.2
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   TASK task_name ( INTERVAL := time_value , PRIORITY := integer_value ) ;

Parameters
----------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Parameter
     - Type
     - Description
   * - ``INTERVAL``
     - ``TIME``
     - Execution interval (cycle time)
   * - ``PRIORITY``
     - ``INT``
     - Task priority (0 = highest)

Behavior
--------

The runtime uses ``INTERVAL`` and ``PRIORITY`` to schedule task execution
during the scan cycle:

- **Interval** controls how often the task runs. On each scheduling round,
  the runtime checks whether the task's interval has elapsed since its last
  execution. If it has, the task is ready to run. A shorter interval means
  the task runs more frequently but consumes more CPU time.

- **Priority** controls execution order when multiple tasks are ready in
  the same round. Priority 0 is the highest. Higher-priority tasks always
  execute before lower-priority tasks. Tasks with equal priority run in
  declaration order.

If a task takes longer than its interval, the runtime skips the missed
cycle and realigns forward. See :doc:`/explanation/execution-cycle` for
details on overruns, watchdog timeouts, and the full scheduling model.

Example
-------

.. code-block::

   RESOURCE DefaultResource ON PLC
       TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
       TASK FastTask(INTERVAL := T#5ms, PRIORITY := 0);

       PROGRAM main WITH MainTask : MainProgram;
   END_RESOURCE

Programs are associated with tasks using the ``WITH`` keyword. A task
executes its associated programs at the specified interval.

See Also
--------

- :doc:`resource` — parent container
- :doc:`configuration` — top-level container
- :doc:`program` — executable unit
