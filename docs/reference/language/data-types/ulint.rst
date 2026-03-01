=====
ULINT
=====

64-bit unsigned integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits
   * - **Range**
     - 0 to 18,446,744,073,709,551,615
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block:: iec61131

   ULINT#42
   ULINT#1000000000
   ULINT#16#FFFFFFFFFFFFFFFF

See Also
--------

- :doc:`udint` — 32-bit unsigned integer
- :doc:`lint` — 64-bit signed integer
