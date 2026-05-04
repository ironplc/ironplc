====
EXPT
====

Returns the result of raising a base to an exponent.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │  EXPT   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION EXPT : ANY_REAL
     VAR_INPUT
       IN1 : ANY_REAL;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN1*. ``EXPT`` accepts ``REAL``,
``LREAL`` for the base and any numeric type for the exponent.

Description
-----------

Returns *IN1* raised to the power *IN2*. ``EXPT(a, b)`` computes
*a*\ :sup:`b`. For integer types, the exponent must be non-negative.
The operator form is ``**``.

Example
-------

.. playground-with-program::
   :vars: result : DINT; value : DINT;

   result := EXPT(2, 10);       (* result = 1024 *)
   value := 3 ** 4;             (* value = 81, operator form *)

See Also
--------

* :doc:`exp` — natural exponential (*e*\ :sup:`x`)
* :doc:`sqrt` — square root
* :doc:`abs` — absolute value

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: EXPT <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_expt.html>`_
* `Beckhoff TwinCAT 3: EXPT <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529122699.html>`_
* `Fernhill SCADA: POW <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
