==
NE
==

Returns TRUE if two inputs are not equal.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   NE    ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION NE : BOOL
     VAR_INPUT
       IN1 : ANY_ELEMENTARY;
       IN2 : ANY_ELEMENTARY;
     END_VAR
   END_FUNCTION

Returns ``BOOL``. ``NE`` accepts ``SINT``, ``INT``, ``DINT``, ``LINT``,
``USINT``, ``UINT``, ``UDINT``, ``ULINT``, ``REAL``, ``LREAL``. Both
inputs must share the same type.

Description
-----------

Returns ``TRUE`` if *IN1* is not equal to *IN2*, ``FALSE`` otherwise.
``NE(a, b)`` is the functional form of the ``<>`` operator: ``a <> b``.
Both forms are equivalent.

For ``REAL`` and ``LREAL`` types, inequality comparison is subject to
floating-point precision limitations.

Example
-------

.. playground-with-program::
   :vars: result : BOOL;

   result := NE(5, 10);    (* result = TRUE *)
   result := 5 <> 10;      (* result = TRUE, operator form *)
   result := 5 <> 5;       (* result = FALSE *)

See Also
--------

* :doc:`eq` — equal
* :doc:`gt` — greater than
* :doc:`lt` — less than

References
----------

* IEC 61131-3 §2.5.1.5.4
* `CODESYS: NE <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_ne.html>`_
* `Beckhoff TwinCAT 3: NE <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/27021600293232779.html>`_
* `Fernhill SCADA: NE <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-comparison.html>`_
