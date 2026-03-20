=======================
BCD_TO_INT / INT_TO_BCD
=======================

Convert between Binary-Coded Decimal (BCD) encoded bit strings and
integer values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.1
   * - **Support**
     - Supported

Signatures
----------

BCD_TO_INT
^^^^^^^^^^

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``BYTE``
     - ``USINT``
     - Supported
   * - 2
     - ``WORD``
     - ``UINT``
     - Supported
   * - 3
     - ``DWORD``
     - ``UDINT``
     - Supported
   * - 4
     - ``LWORD``
     - ``ULINT``
     - Supported

INT_TO_BCD
^^^^^^^^^^

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``USINT``
     - ``BYTE``
     - Supported
   * - 2
     - ``UINT``
     - ``WORD``
     - Supported
   * - 3
     - ``UDINT``
     - ``DWORD``
     - Supported
   * - 4
     - ``ULINT``
     - ``LWORD``
     - Supported

Description
-----------

BCD (Binary-Coded Decimal) encodes each decimal digit in a 4-bit nibble.
For example, the decimal value ``42`` is encoded as ``0100_0010`` in BCD
(``4`` in the high nibble, ``2`` in the low nibble).

``BCD_TO_INT`` decodes a BCD-encoded bit string into its integer value.
The function treats invalid BCD nibbles (values 10--15) as 0.

``INT_TO_BCD`` encodes an integer value into BCD format. Values that
exceed the maximum representable BCD value for the target width wrap
around.

Maximum values per width:

- ``BYTE``: 99
- ``WORD``: 9999
- ``DWORD``: 99999999
- ``LWORD``: 9999999999999999

Example
-------

.. playground-with-program::
   :vars: bcd_val : BYTE; int_val : USINT;

   int_val := BCD_TO_INT(BYTE#16#42);  (* int_val = 42 *)
   bcd_val := INT_TO_BCD(USINT#42);    (* bcd_val = 16#42 *)

See Also
--------

- :doc:`type-conversions` — other type conversion functions
