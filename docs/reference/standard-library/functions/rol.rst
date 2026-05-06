===
ROL
===

Rotates a bit string left by a specified number of positions.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │   ROL   ├─ OUT
        N ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION ROL : ANY_BIT
     VAR_INPUT
       IN : ANY_BIT;
       N  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``ROL`` accepts ``BYTE``,
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
     - Number of positions to rotate left.

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
     - IN rotated left by N positions, with bits wrapping from the leftmost to the rightmost position. Same type as IN.

Description
-----------

Rotates the bit string *IN* left by *N* positions. Bits shifted out
of the leftmost position wrap around to the rightmost position. No
bits are lost.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := ROL(WORD#16#F000, 4);        (* result = 16#000F *)

See Also
--------

* :doc:`ror` — rotate right
* :doc:`shl` — shift left
* :doc:`shr` — shift right

References
----------

* IEC 61131-3 §2.5.1.5.6
* `CODESYS: ROL <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_rol.html>`_
* `Beckhoff TwinCAT 3: ROL <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528945291.html>`_
* `Fernhill SCADA: ROL <https://www.fernhillsoftware.com/help/iec-61131/common-elements/bitshift-functions/rotate-left.html>`_
