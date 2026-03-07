===
SHR
===

Shifts a bit string right by a specified number of positions.

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

Shifts the bit string *IN* right by *N* positions. Vacated positions
on the left are filled with zeros. Bits shifted beyond the rightmost
position are discarded.

Example
-------

.. playground::
   :vars: result : WORD;

   result := SHR(WORD#16#FF00, 8);        (* result = 16#00FF *)

See Also
--------

- :doc:`shl` — shift left
- :doc:`ror` — rotate right
- :doc:`rol` — rotate left
