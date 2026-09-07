===
AND
===

Returns the bitwise AND of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤   AND   ├─ OUT
       IN3 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION AND : ANY_BIT
     VAR_INPUT
       IN1 : ANY_BIT;
       IN2 : ANY_BIT;
       (* ... additional inputs ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``AND`` accepts ``BOOL``,
``BYTE``, ``WORD``, ``DWORD``, ``LWORD``. All inputs must share the same
type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_BIT``
     - The first operand.
   * - ``IN2``
     - ``ANY_BIT``
     - The second operand.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_BIT``
     - Each bit set in every input. Same type as the inputs.

Description
-----------

On ``BOOL`` inputs, returns ``TRUE`` only when every input is ``TRUE``.
On ``BYTE``, ``WORD``, ``DWORD`` and ``LWORD`` inputs, each bit of the
result is set only when the same bit is set in every input. ``AND(a, b)``
is the functional form of the ``AND`` operator: ``a AND b``. Both forms are
equivalent, and ``AND(a, b, c)`` is ``a AND b AND c``.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := AND(WORD#16#F0F0, WORD#16#FF00);   (* result = 16#F000 *)
   result := WORD#16#F0F0 AND WORD#16#FF00;     (* operator form *)
   result := AND(WORD#16#F0F0, WORD#16#FF00, WORD#16#0FF0);   (* result = 16#0000 *)

See Also
--------

* :doc:`or` — bitwise OR
* :doc:`xor` — bitwise exclusive OR
* :doc:`not` — bitwise complement
* :doc:`/reference/language/structured-text/logical-operators` — the operator forms

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: AND <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_and.html>`_
