=====
DWORD
=====

Bit string of 32 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits
   * - **Range**
     - 16#00000000 to 16#FFFFFFFF
   * - **Default**
     - 16#00000000
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   DWORD#16#DEADBEEF
   DWORD#16#00FF00FF

See Also
--------

- :doc:`word` — 16-bit bit string
- :doc:`lword` — 64-bit bit string
- :doc:`udint` — 32-bit unsigned integer
