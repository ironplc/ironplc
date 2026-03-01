===
ROL
===

Rotates a bit string left by a specified number of positions.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.6
   * - **Support**
     - Not yet supported

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
     - Not yet supported
   * - 2
     - ``WORD``
     - ``INT``
     - ``WORD``
     - Not yet supported
   * - 3
     - ``DWORD``
     - ``INT``
     - ``DWORD``
     - Not yet supported
   * - 4
     - ``LWORD``
     - ``INT``
     - ``LWORD``
     - Not yet supported

Description
-----------

Rotates the bit string *IN* left by *N* positions. Bits shifted out
of the leftmost position wrap around to the rightmost position. No
bits are lost.

Example
-------

.. code-block::

   result := ROL(BYTE#2#1000_0001, 1);   (* result = 2#0000_0011 *)
   result := ROL(WORD#16#F000, 4);        (* result = 16#000F *)

See Also
--------

- :doc:`ror` — rotate right
- :doc:`shl` — shift left
- :doc:`shr` — shift right
