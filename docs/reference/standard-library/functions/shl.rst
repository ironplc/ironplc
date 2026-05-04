===
SHL
===

Shifts a bit string left by a specified number of positions.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │   SHL   ├─ OUT
        N ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION SHL : ANY_BIT
     VAR_INPUT
       IN : ANY_BIT;
       N  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``SHL`` accepts ``BYTE``,
``WORD``, ``DWORD``, ``LWORD`` for *IN*; *N* is ``INT``.

Description
-----------

Shifts the bit string *IN* left by *N* positions. Vacated positions
on the right are filled with zeros. Bits shifted beyond the leftmost
position are discarded.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := SHL(WORD#16#00FF, 8);        (* result = 16#FF00 *)

See Also
--------

* :doc:`shr` — shift right
* :doc:`rol` — rotate left
* :doc:`ror` — rotate right

References
----------

* IEC 61131-3 §2.5.1.5.6
* `CODESYS: SHL <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_shl.html>`_
* `Beckhoff TwinCAT 3: SHL <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528934539.html>`_
* `Fernhill SCADA: SHL <https://www.fernhillsoftware.com/help/iec-61131/common-elements/bitshift-functions/shift-left.html>`_
