=======================
BCD_TO_INT / INT_TO_BCD
=======================

Convert between Binary-Coded Decimal (BCD) encoded bit strings and
integer values.

Signature
---------

``BCD_TO_INT``:

.. code-block:: text

           ┌─────────────┐
       IN ─┤  BCD_TO_INT ├─ OUT
           └─────────────┘

.. code-block:: text

   FUNCTION BCD_TO_INT : ANY_INT
     VAR_INPUT
       IN : ANY_BIT;
     END_VAR
   END_FUNCTION

``BCD_TO_INT`` accepts ``BYTE``, ``WORD``, ``DWORD``, ``LWORD`` for
*IN*; the return type is the corresponding unsigned integer (``USINT``,
``UINT``, ``UDINT``, ``ULINT``).

``INT_TO_BCD``:

.. code-block:: text

           ┌─────────────┐
       IN ─┤  INT_TO_BCD ├─ OUT
           └─────────────┘

.. code-block:: text

   FUNCTION INT_TO_BCD : ANY_BIT
     VAR_INPUT
       IN : ANY_INT;
     END_VAR
   END_FUNCTION

``INT_TO_BCD`` accepts ``USINT``, ``UINT``, ``UDINT``, ``ULINT`` for
*IN*; the return type is the corresponding bit string (``BYTE``,
``WORD``, ``DWORD``, ``LWORD``).

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

* :doc:`type-conversions` — other type conversion functions

References
----------

* IEC 61131-3 §2.5.1.5.1
* `CODESYS: Operators (overview) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_struct_reference_operators.html>`_
* `Beckhoff TwinCAT 3: Type conversion (overview) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/63050398781277579.html>`_
* `Fernhill SCADA: BCD_TO_INT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/conversion-functions/bcd-to-integer.html>`_
* `Fernhill SCADA: INT_TO_BCD <https://www.fernhillsoftware.com/help/iec-61131/common-elements/conversion-functions/integer-to-bcd.html>`_
