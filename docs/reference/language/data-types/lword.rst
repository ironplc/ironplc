=====
LWORD
=====

Bit string of 64 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits
   * - **Range**
     - 16#0000000000000000 to 16#FFFFFFFFFFFFFFFF
   * - **Default**
     - 16#0000000000000000
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   LWORD#16#FFFFFFFFFFFFFFFF
   LWORD#16#0000000000000001

See Also
--------

- :doc:`dword` — 32-bit bit string
- :doc:`ulint` — 64-bit unsigned integer
