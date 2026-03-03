===
SHL
===

Shifts a bit string left by a specified number of positions.

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

Shifts the bit string *IN* left by *N* positions. Vacated positions
on the right are filled with zeros. Bits shifted beyond the leftmost
position are discarded.

Example
-------

.. code-block::

   result := SHL(BYTE#2#0000_0001, 3);   (* result = 2#0000_1000 *)
   result := SHL(WORD#16#00FF, 8);        (* result = 16#FF00 *)

See Also
--------

- :doc:`shr` — shift right
- :doc:`rol` — rotate left
- :doc:`ror` — rotate right
