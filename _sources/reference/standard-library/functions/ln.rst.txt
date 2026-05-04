==
LN
==

Returns the natural logarithm of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   LN    ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION LN : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``LN`` accepts ``REAL``,
``LREAL``.

Description
-----------

Returns the natural logarithm (base *e*) of *IN*. The input must be
positive; the result of ``LN`` applied to zero or a negative value
is undefined.

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := LN(REAL#2.718282);  (* result ~ 1.0 *)
   value := LN(LREAL#1.0);      (* value = 0.0 *)

See Also
--------

* :doc:`log` — base-10 logarithm
* :doc:`exp` — natural exponential

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: LN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_ln.html>`_
* `Beckhoff TwinCAT 3: LN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529106571.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
