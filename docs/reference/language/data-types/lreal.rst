=====
LREAL
=====

64-bit double-precision IEEE 754 floating-point number.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits
   * - **Range**
     - Approximately -1.8E+308 to 1.8E+308
   * - **Precision**
     - ~15 decimal digits
   * - **Default**
     - 0.0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   LREAL#3.14159265358979
   LREAL#-1.0
   LREAL#1.0E+100

Example
-------

.. playground-with-program::
   :vars: precise_value : LREAL; correction : LREAL; result : LREAL;

   precise_value := LREAL#3.14159265358979;
   correction := LREAL#0.00000000000001;
   result := precise_value + correction;

See Also
--------

- :doc:`real` — 32-bit single-precision floating point
