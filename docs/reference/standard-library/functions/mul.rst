===
MUL
===

Returns the product of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤   MUL   ├─ OUT
       IN3 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MUL : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
       (* ... additional inputs ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``MUL`` accepts ``SINT``,
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
     - The first multiplicand.
   * - ``IN2``
     - ``ANY_NUM``
     - The second multiplicand.

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
     - The product of all inputs. Same type as the inputs.

Description
-----------

Returns the product of all inputs. ``MUL(a, b)`` is the functional form
of the ``*`` operator: ``a * b``. Both forms are equivalent, and
``MUL(a, b, c)`` is ``a * b * c``.

For integer types, overflow behavior wraps around (modular arithmetic).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MUL(6, 7);      (* result = 42 *)
   result := MUL(2, 3, 7);   (* result = 42 *)
   result := 6 * 7;          (* result = 42, operator form *)

See Also
--------

* :doc:`add` — addition
* :doc:`div` — division
* :doc:`mod` — modulo

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: MUL <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_mul.html>`_
* `Beckhoff TwinCAT 3: MUL <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528864651.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
