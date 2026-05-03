===
ROR
===

Rotates a bit string right by a specified number of positions.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.6
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 20 20 20

   * - #
     - Input (IN)
     - Input (N)
     - Return Type
   * - 1
     - ``BYTE``
     - ``INT``
     - ``BYTE``
   * - 2
     - ``WORD``
     - ``INT``
     - ``WORD``
   * - 3
     - ``DWORD``
     - ``INT``
     - ``DWORD``
   * - 4
     - ``LWORD``
     - ``INT``
     - ``LWORD``

Description
-----------

Rotates the bit string *IN* right by *N* positions. Bits shifted out
of the rightmost position wrap around to the leftmost position. No
bits are lost.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := ROR(WORD#16#000F, 4);        (* result = 16#F000 *)

See Also
--------

- :doc:`rol` — rotate left
- :doc:`shr` — shift right
- :doc:`shl` — shift left
