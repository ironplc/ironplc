========
RESOURCE
========

A resource represents a processing unit within a configuration, typically
corresponding to a CPU or processing module.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.7.1
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   RESOURCE resource_name ON resource_type
       task_declarations
       program_associations
       global_variable_declarations
   END_RESOURCE

Example
-------

.. code-block::

   RESOURCE DefaultResource ON PLC
       TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
       TASK FastTask(INTERVAL := T#5ms, PRIORITY := 0);

       PROGRAM main WITH MainTask : MainProgram;
       PROGRAM fast WITH FastTask : FastProgram;
   END_RESOURCE

A resource contains task declarations and associates programs with
those tasks.

See Also
--------

- :doc:`configuration` — parent container
- :doc:`task` — execution scheduling
- :doc:`program` — executable unit
