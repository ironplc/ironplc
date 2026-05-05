==
LT
==

Returns TRUE if the first input is less than the second.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   LT    ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION LT : BOOL
     VAR_INPUT
       IN1 : ANY_ELEMENTARY;
       IN2 : ANY_ELEMENTARY;
     END_VAR
   END_FUNCTION

Returns ``BOOL``. ``LT`` accepts ``SINT``, ``INT``, ``DINT``, ``LINT``,
``USINT``, ``UINT``, ``UDINT``, ``ULINT``, ``REAL``, ``LREAL``. Both
inputs must share the same type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_MAGNITUDE``
     - The first value to compare.
   * - ``IN2``
     - ``ANY_MAGNITUDE``
     - The second value to compare.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``BOOL``
     - TRUE if IN1 is strictly less than IN2, otherwise FALSE.

Description
-----------

Returns ``TRUE`` if *IN1* is strictly less than *IN2*, ``FALSE``
otherwise. ``LT(a, b)`` is the functional form of the ``<`` operator:
``a < b``. Both forms are equivalent.

Example
-------

.. playground-with-program::
   :vars: result : BOOL;

   result := LT(5, 10);    (* result = TRUE *)
   result := 5 < 10;       (* result = TRUE, operator form *)
   result := 5 < 5;        (* result = FALSE *)

See Also
--------

* :doc:`le` — less than or equal
* :doc:`gt` — greater than
* :doc:`eq` — equal

References
----------

* IEC 61131-3 §2.5.1.5.4
* `CODESYS: LT <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_lt.html>`_
* `Beckhoff TwinCAT 3: LT <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528988299.html>`_
* `Fernhill SCADA: LT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-comparison.html>`_
