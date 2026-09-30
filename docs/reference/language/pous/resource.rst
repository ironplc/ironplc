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
       global_variable_declarations
       task_declarations
       program_associations
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

.. include:: /includes/single-program-limitation.rst

Resource Global Variables
-------------------------

A resource may declare its own :code:`VAR_GLOBAL` block before its tasks.
These global variables belong to the resource: a program that the resource
instantiates accesses them through :code:`VAR_EXTERNAL`, the same way as
configuration globals (see :doc:`/reference/language/variables/scope`). Their
values persist from one scan to the next.

.. playground::

   CONFIGURATION config
     RESOURCE resource1 ON PLC
       VAR_GLOBAL
         ScanCount : DINT := 0;
       END_VAR
       TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
       PROGRAM plc_task_instance WITH plc_task : main;
     END_RESOURCE
   END_CONFIGURATION

   PROGRAM main
     VAR_EXTERNAL
       ScanCount : DINT;
     END_VAR
     ScanCount := ScanCount + 1;
   END_PROGRAM

IEC 61131-3 makes the global variables of a resource visible only to the
programs of that resource, while configuration globals are visible to every
resource.

See Also
--------

- :doc:`configuration` — parent container
- :doc:`task` — execution scheduling
- :doc:`program` — executable unit
