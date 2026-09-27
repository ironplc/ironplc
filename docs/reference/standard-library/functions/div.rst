===
DIV
===

Returns the quotient of two inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   DIV   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION DIV : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``DIV`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. Both inputs must share the same type.

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
     - The dividend.
   * - ``IN2``
     - ``ANY_NUM``
     - The divisor. Must be non-zero.

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
     - IN1 divided by IN2. Same type as the inputs.

Description
-----------

Returns *IN1* divided by *IN2*. ``DIV(a, b)`` is the functional form
of the ``/`` operator: ``a / b``. Both forms are equivalent.

For integer types, division truncates toward zero. Division by zero
causes a runtime fault.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := DIV(42, 6);   (* result = 7 *)
   result := 42 / 6;       (* result = 7, operator form *)
   result := 7 / 2;        (* result = 3, truncates toward zero *)

See Also
--------

* :doc:`mul` — multiplication
* :doc:`mod` — modulo
* :doc:`sub` — subtraction

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: DIV <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_div.html>`_
* `Beckhoff TwinCAT 3: DIV <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528875403.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
