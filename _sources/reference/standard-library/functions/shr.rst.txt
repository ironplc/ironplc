===
SHR
===

Shifts a bit string right by a specified number of positions.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │   SHR   ├─ OUT
        N ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION SHR : ANY_BIT
     VAR_INPUT
       IN : ANY_BIT;
       N  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``SHR`` accepts ``BYTE``,
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
     - The bit string to shift.
   * - ``N``
     - ``ANY_INT``
     - Number of positions to shift right.

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
     - IN shifted right by N positions, with zeros filled in on the left. Same type as IN.

Description
-----------

Shifts the bit string *IN* right by *N* positions. Vacated positions
on the left are filled with zeros. Bits shifted beyond the rightmost
position are discarded.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := SHR(WORD#16#FF00, 8);        (* result = 16#00FF *)

See Also
--------

* :doc:`shl` — shift left
* :doc:`ror` — rotate right
* :doc:`rol` — rotate left

References
----------

* IEC 61131-3 §2.5.1.5.6
* `CODESYS: SHR <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_shr.html>`_
* `Beckhoff TwinCAT 3: SHR <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528939915.html>`_
* `Fernhill SCADA: SHR <https://www.fernhillsoftware.com/help/iec-61131/common-elements/bitshift-functions/shift-right.html>`_
