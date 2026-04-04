=================
Extension Library
=================

IronPLC provides vendor extension functions, function blocks, and variables
that go beyond the IEC 61131-3 standard. These extensions are commonly
supported by other PLC environments such as CODESYS, TwinCAT, and RuSTy.

.. tip::

   Extension library items require explicit opt-in via ``--allow-*`` flags
   or ``--dialect rusty``. See :doc:`/explanation/enabling-dialects-and-features`
   for details.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`SIZEOF <functions/sizeof>`
     - Size in bytes of a variable or type (requires ``--allow-sizeof``)

Function Blocks
---------------

No vendor extension function blocks are currently defined.

Variables
---------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Variable
     - Description
   * - :doc:`__SYSTEM_UP_TIME / __SYSTEM_UP_LTIME <variables/system-uptime>`
     - Monotonic uptime since VM start (requires ``--allow-system-uptime-global``)

.. toctree::
   :maxdepth: 1
   :hidden:

   functions/index
   function-blocks/index
   variables/index
