==============
I/O Qualifiers
==============

Direct representation allows variables to be mapped to specific locations
in the process image using address prefixes.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.1.1
   * - **Support**
     - Partial

Address Prefixes
----------------

.. list-table::
   :header-rows: 1
   :widths: 15 25 60

   * - Prefix
     - Region
     - Description
   * - ``%I``
     - Input
     - Read from physical inputs
   * - ``%Q``
     - Output
     - Write to physical outputs
   * - ``%M``
     - Memory
     - Internal memory (markers)

Size Prefixes
-------------

.. list-table::
   :header-rows: 1
   :widths: 15 25 60

   * - Prefix
     - Size
     - Description
   * - ``X``
     - 1 bit
     - Single bit (default)
   * - ``B``
     - 8 bits
     - Byte
   * - ``W``
     - 16 bits
     - Word
   * - ``D``
     - 32 bits
     - Double word
   * - ``L``
     - 64 bits
     - Long word

Syntax
------

.. code-block:: bnf

   variable_name AT %prefix.address : type_name ;

Example
-------

.. code-block::

   PROGRAM main
       VAR
           start_button AT %IX0.0 : BOOL;
           motor_output AT %QX0.0 : BOOL;
           speed_setpoint AT %MW10 : INT;
       END_VAR

       motor_output := start_button;
   END_PROGRAM

Located Global Variables
------------------------

A :code:`VAR_GLOBAL` block of a :code:`CONFIGURATION` can locate its
variables the same way. Programs reach them through :code:`VAR_EXTERNAL`,
which names the variable without its address:

.. playground::

   CONFIGURATION config
     VAR_GLOBAL
       start_button AT %IX0.0 : BOOL;
       motor_output AT %QX0.0 : BOOL;
       scan_count AT %MW2 : INT;
     END_VAR
     RESOURCE resource1 ON PLC
       TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
       PROGRAM plc_task_instance WITH plc_task : main;
     END_RESOURCE
   END_CONFIGURATION

   PROGRAM main
     VAR_EXTERNAL
       start_button : BOOL;
       motor_output : BOOL;
       scan_count : INT;
     END_VAR
     motor_output := start_button;
     scan_count := scan_count + 1;
   END_PROGRAM

See Also
--------

- :doc:`declarations` — basic variable declarations
- :doc:`scope` — variable scope keywords
