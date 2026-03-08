====
REAL
====

32-bit single-precision IEEE 754 floating-point number.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits
   * - **Range**
     - Approximately -3.4E+38 to 3.4E+38
   * - **Precision**
     - ~7 decimal digits
   * - **Default**
     - 0.0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   REAL#3.14
   REAL#-1.0
   REAL#1.0E+10
   REAL#2.5E-3

Example
-------

.. playground-with-program::
   :vars: raw_temp : REAL; scale : REAL; celsius : REAL;

   raw_temp := REAL#2048.0;
   scale := REAL#0.1;
   celsius := raw_temp * scale;  (* celsius = 204.8 *)

See Also
--------

- :doc:`lreal` — 64-bit double-precision floating point
