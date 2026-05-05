==
LE
==

Returns TRUE if the first input is less than or equal to the second.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   LE    ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION LE : BOOL
     VAR_INPUT
       IN1 : ANY_ELEMENTARY;
       IN2 : ANY_ELEMENTARY;
     END_VAR
   END_FUNCTION

Returns ``BOOL``. ``LE`` accepts ``SINT``, ``INT``, ``DINT``, ``LINT``,
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
     - TRUE if IN1 is less than or equal to IN2, otherwise FALSE.

Description
-----------

Returns ``TRUE`` if *IN1* is less than or equal to *IN2*, ``FALSE``
otherwise. ``LE(a, b)`` is the functional form of the ``<=`` operator:
``a <= b``. Both forms are equivalent.

Example
-------

.. playground-with-program::
   :vars: result : BOOL;

   result := LE(5, 10);    (* result = TRUE *)
   result := 5 <= 10;      (* result = TRUE, operator form *)
   result := 5 <= 5;       (* result = TRUE *)

See Also
--------

* :doc:`lt` — less than
* :doc:`ge` — greater than or equal
* :doc:`eq` — equal

References
----------

* IEC 61131-3 §2.5.1.5.4
* `CODESYS: LE <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_le.html>`_
* `Beckhoff TwinCAT 3: LE <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528993675.html>`_
* `Fernhill SCADA: LE <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-comparison.html>`_
