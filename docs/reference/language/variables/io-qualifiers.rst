=============
I/O Qualifiers
=============

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

.. code-block:: iec61131

   PROGRAM main
       VAR
           start_button AT %IX0.0 : BOOL;
           motor_output AT %QX0.0 : BOOL;
           speed_setpoint AT %MW10 : INT;
       END_VAR

       motor_output := start_button;
   END_PROGRAM

See Also
--------

- :doc:`declarations` — basic variable declarations
- :doc:`scope` — variable scope keywords
