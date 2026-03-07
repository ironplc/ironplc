===
ROL
===

Rotates a bit string left by a specified number of positions.

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
   :widths: 10 20 20 20 30

   * - #
     - Input (IN)
     - Input (N)
     - Return Type
     - Support
   * - 1
     - ``BYTE``
     - ``INT``
     - ``BYTE``
     - Supported
   * - 2
     - ``WORD``
     - ``INT``
     - ``WORD``
     - Supported
   * - 3
     - ``DWORD``
     - ``INT``
     - ``DWORD``
     - Supported
   * - 4
     - ``LWORD``
     - ``INT``
     - ``LWORD``
     - Supported

Description
-----------

Rotates the bit string *IN* left by *N* positions. Bits shifted out
of the leftmost position wrap around to the rightmost position. No
bits are lost.

Example
-------

.. playground::
   :vars: result : WORD;

   result := ROL(WORD#16#F000, 4);        (* result = 16#000F *)

See Also
--------

- :doc:`ror` — rotate right
- :doc:`shl` — shift left
- :doc:`shr` — shift right
