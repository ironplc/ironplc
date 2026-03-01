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

Example
-------

.. code-block:: iec61131

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
