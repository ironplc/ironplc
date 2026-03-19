===========================
Program Organization Units
===========================

IEC 61131-3 programs are structured into program organization units (POUs):
programs, function blocks, and functions. These are deployed within a
configuration containing resources and tasks.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Unit
     - Description
   * - :doc:`program`
     - Top-level executable unit
   * - :doc:`function`
     - Stateless callable unit with return value
   * - :doc:`function-block`
     - Stateful callable unit with inputs and outputs
   * - :doc:`configuration`
     - Top-level deployment container
   * - :doc:`resource`
     - Processing resource within a configuration
   * - :doc:`task`
     - Execution scheduling unit

.. toctree::
   :maxdepth: 1
   :hidden:

   program
   function
   function-block
   configuration
   resource
   task
