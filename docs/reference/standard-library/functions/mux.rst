===
MUX
===

Multiplexer — selects one of several inputs by index.

Signature
---------

.. code-block:: text

            ┌─────────┐
         K ─┤         │
       IN0 ─┤         │
       IN1 ─┤   MUX   ├─ OUT
       IN2 ─┤         │
       ... ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MUX : ANY
     VAR_INPUT
       K   : INT;
       IN0 : ANY;
       IN1 : ANY;
       (* ... up to IN15 ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. All ``INn`` inputs must share
the same type. ``MUX`` is polymorphic over any data type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``K``
     - ``ANY_INT``
     - Zero-based selector. Selects which input is returned.
   * - ``IN0``, ``IN1``, ..., ``INn``
     - ``ANY``
     - The candidate values. The number of inputs matches the value range of K (2 to 16). All inputs must share the same type.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY``
     - The input selected by K. Same type as the INn inputs.

Description
-----------

``MUX(K, IN0, IN1, ...)`` returns the input selected by the zero-based
index *K*. The number of inputs is variable, and all inputs must be
the same type.

- If *K* = 0, returns *IN0*
- If *K* = 1, returns *IN1*
- And so on

If *K* is out of range, the value is clamped: negative *K* selects
*IN0*, and *K* greater than or equal to the number of inputs selects
the last input. Supports 2 to 16 input values.

This function is polymorphic: it works with any data type for the
selected inputs.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MUX(0, 10, 20, 30);    (* result = 10 *)
   result := MUX(2, 10, 20, 30);    (* result = 30 *)

See Also
--------

* :doc:`sel` — binary selection (two inputs)
* :doc:`limit` — clamp to range

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: MUX <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_mux.html>`_
* `Beckhoff TwinCAT 3: MUX <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528977547.html>`_
* `Fernhill SCADA: MUX <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/mux.html>`_
