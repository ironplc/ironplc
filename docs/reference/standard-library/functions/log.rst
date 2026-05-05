===
LOG
===

Returns the base-10 logarithm of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   LOG   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION LOG : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``LOG`` accepts ``REAL``,
``LREAL``.

Description
-----------

Returns the common logarithm (base 10) of *IN*. The input must be
positive; the result of ``LOG`` applied to zero or a negative value
is undefined.

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := LOG(REAL#100.0);  (* result = 2.0 *)
   value := LOG(LREAL#1000.0); (* value = 3.0 *)

See Also
--------

* :doc:`ln` — natural logarithm
* :doc:`exp` — natural exponential

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: LOG <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_log.html>`_
* `Beckhoff TwinCAT 3: LOG <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529111947.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
