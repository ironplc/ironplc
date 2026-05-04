===
EXP
===

Returns the natural exponential of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   EXP   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION EXP : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``EXP`` accepts ``REAL``,
``LREAL``.

Description
-----------

Returns *e* raised to the power of *IN*, where *e* is Euler's number
(approximately 2.71828). This is the inverse of the :doc:`ln` function.

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := EXP(REAL#1.0);   (* result ~ 2.718282 *)
   value := EXP(LREAL#0.0);   (* value = 1.0 *)

See Also
--------

* :doc:`ln` — natural logarithm
* :doc:`expt` — exponentiation

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: EXP <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_exp.html>`_
* `Beckhoff TwinCAT 3: EXP <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529117323.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
