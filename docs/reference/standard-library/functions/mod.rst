===
MOD
===

Returns the remainder after integer division.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   MOD   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MOD : ANY_INT
     VAR_INPUT
       IN1 : ANY_INT;
       IN2 : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``MOD`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``.
Both inputs must share the same type.

Description
-----------

Returns the remainder of *IN1* divided by *IN2*. ``MOD(a, b)`` is the
functional form of the ``MOD`` operator: ``a MOD b``. Both forms are
equivalent.

The result has the same sign as *IN1*. IEC 61131-3 defines the ``MOD``
function only for integer types. Division by zero causes a runtime fault.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MOD(7, 3);    (* result = 1 *)
   result := 7 MOD 3;      (* result = 1, operator form *)
   result := -7 MOD 3;     (* result = -1 *)

See Also
--------

* :doc:`div` — division
* :doc:`mul` — multiplication

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: MOD <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_mod.html>`_
* `Beckhoff TwinCAT 3: MOD <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528880779.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
