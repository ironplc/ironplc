===
MAX
===

Returns the larger of two inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   MAX   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MAX : ANY
     VAR_INPUT
       IN1 : ANY;
       IN2 : ANY;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``MAX`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. Both inputs must share the same type.

Description
-----------

Returns the larger of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MAX(10, 20);    (* result = 20 *)
   result := MAX(-5, 3);     (* result = 3 *)

See Also
--------

* :doc:`min` — minimum of two values
* :doc:`limit` — clamp to range
* :doc:`sel` — binary selection

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: MAX <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_max.html>`_
* `Beckhoff TwinCAT 3: MAX <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528961419.html>`_
* `Fernhill SCADA: MAX <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/maximum.html>`_
