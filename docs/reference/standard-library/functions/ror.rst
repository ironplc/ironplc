===
ROR
===

Rotates a bit string right by a specified number of positions.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │   ROR   ├─ OUT
        N ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION ROR : ANY_BIT
     VAR_INPUT
       IN : ANY_BIT;
       N  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``ROR`` accepts ``BYTE``,
``WORD``, ``DWORD``, ``LWORD`` for *IN*; *N* is ``INT``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_BIT``
     - The bit string to rotate.
   * - ``N``
     - ``ANY_INT``
     - Number of positions to rotate right.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_BIT``
     - IN rotated right by N positions, with bits wrapping from the rightmost to the leftmost position. Same type as IN.

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

* :doc:`rol` — rotate left
* :doc:`shr` — shift right
* :doc:`shl` — shift left

References
----------

* IEC 61131-3 §2.5.1.5.6
* `CODESYS: ROR <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_ror.html>`_
* `Beckhoff TwinCAT 3: ROR <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528950667.html>`_
* `Fernhill SCADA: ROR <https://www.fernhillsoftware.com/help/iec-61131/common-elements/bitshift-functions/rotate-right.html>`_
