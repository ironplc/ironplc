===
SUB
===

Returns the difference of two inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   SUB   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION SUB : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``SUB`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. Both inputs must share the same type.

Description
-----------

Returns *IN1* minus *IN2*. ``SUB(a, b)`` is the functional form of the
``-`` operator: ``a - b``. Both forms are equivalent.

For integer types, underflow behavior wraps around (modular arithmetic).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := SUB(30, 10);   (* result = 20 *)
   result := 30 - 10;       (* result = 20, operator form *)

See Also
--------

* :doc:`add` — addition
* :doc:`mul` — multiplication
* :doc:`div` — division

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: SUB <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
