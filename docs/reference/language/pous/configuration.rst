=============
CONFIGURATION
=============

A configuration is the top-level deployment container that describes
how programs are assigned to resources and tasks.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.7.1
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   CONFIGURATION configuration_name
       resource_declarations
       global_variable_declarations
   END_CONFIGURATION

Example
-------

.. code-block:: iec61131

   CONFIGURATION DefaultConfig
       RESOURCE DefaultResource ON PLC
           TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
           PROGRAM main WITH MainTask : MainProgram;
       END_RESOURCE
   END_CONFIGURATION

A configuration contains one or more resources. Global variables
declared at configuration level are accessible to all resources.

See Also
--------

- :doc:`resource` — processing resource
- :doc:`task` — execution scheduling
- :doc:`program` — executable unit
