===========================
Program Organization Units
===========================

IEC 61131-3 programs are structured into program organization units (POUs):
programs, function blocks, and functions. These are deployed within a
configuration containing resources and tasks.

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Unit
     - Description
     - Status
   * - :doc:`program`
     - Top-level executable unit
     - Supported
   * - :doc:`function`
     - Stateless callable unit with return value
     - Partial
   * - :doc:`function-block`
     - Stateful callable unit with inputs and outputs
     - Partial
   * - :doc:`configuration`
     - Top-level deployment container
     - Supported
   * - :doc:`resource`
     - Processing resource within a configuration
     - Supported
   * - :doc:`task`
     - Execution scheduling unit
     - Supported

.. toctree::
   :maxdepth: 1
   :hidden:

   program
   function
   function-block
   configuration
   resource
   task
