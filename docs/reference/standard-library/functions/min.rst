===
MIN
===

Returns the smaller of two inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   MIN   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MIN : ANY
     VAR_INPUT
       IN1 : ANY;
       IN2 : ANY;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``MIN`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. Both inputs must share the same type.

Description
-----------

Returns the smaller of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MIN(10, 20);    (* result = 10 *)
   result := MIN(-5, 3);     (* result = -5 *)

See Also
--------

* :doc:`max` — maximum of two values
* :doc:`limit` — clamp to range
* :doc:`sel` — binary selection

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: MIN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_min.html>`_
* `Beckhoff TwinCAT 3: MIN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528966795.html>`_
* `Fernhill SCADA: MIN <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/minimum.html>`_
