=====
UDINT
=====

32-bit unsigned integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits
   * - **Range**
     - 0 to 4,294,967,295
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   UDINT#42
   UDINT#1000000
   UDINT#16#FFFFFFFF

Example
-------

.. playground-with-program::
   :vars: total_units : UDINT; batch : UDINT; running_total : UDINT;

   total_units := UDINT#50000;
   batch := UDINT#1200;
   running_total := total_units + batch;  (* running_total = 51200 *)

See Also
--------

- :doc:`uint` — 16-bit unsigned integer
- :doc:`ulint` — 64-bit unsigned integer
- :doc:`dint` — 32-bit signed integer
