===
ADD
===

Returns the sum of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤   ADD   ├─ OUT
       IN3 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION ADD : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
       (* ... additional inputs ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ADD`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. All inputs must share the same type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_NUM``
     - The first addend.
   * - ``IN2``
     - ``ANY_NUM``
     - The second addend.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_NUM``
     - The sum of IN1 and IN2. Same type as the inputs.

Description
-----------

Returns the sum of *IN1* and *IN2*. ``ADD(a, b)`` is the functional
form of the ``+`` operator: ``a + b``. Both forms are equivalent.

For integer types, overflow behavior wraps around (modular arithmetic).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := ADD(10, 20);   (* result = 30 *)
   result := 10 + 20;       (* result = 30, operator form *)

See Also
--------

* :doc:`sub` — subtraction
* :doc:`mul` — multiplication
* :doc:`div` — division

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: ADD <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_add.html>`_
* `Beckhoff TwinCAT 3: ADD <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038341259.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
