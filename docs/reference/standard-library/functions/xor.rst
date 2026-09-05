===
XOR
===

Returns the bitwise exclusive OR of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤   XOR   ├─ OUT
       IN3 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION XOR : ANY_BIT
     VAR_INPUT
       IN1 : ANY_BIT;
       IN2 : ANY_BIT;
       (* ... additional inputs ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``XOR`` accepts ``BOOL``,
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
     - Each bit set in an odd number of the inputs. Same type as the inputs.

Description
-----------

On ``BOOL`` inputs, returns ``TRUE`` when an odd number of the inputs are
``TRUE``: for two inputs, when exactly one is. On ``BYTE``, ``WORD``,
``DWORD`` and ``LWORD`` inputs, each bit of the result is set when the same
bit is set in an odd number of the inputs. ``XOR(a, b)`` is the functional
form of the ``XOR`` operator: ``a XOR b``. Both forms are equivalent, and
``XOR(a, b, c)`` is ``a XOR b XOR c``.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := XOR(WORD#16#F0F0, WORD#16#FF00);   (* result = 16#0FF0 *)
   result := WORD#16#F0F0 XOR WORD#16#FF00;     (* operator form *)
   result := XOR(WORD#16#F0F0, WORD#16#FF00, WORD#16#00FF);   (* result = 16#0F0F *)

See Also
--------

* :doc:`and` — bitwise AND
* :doc:`or` — bitwise OR
* :doc:`not` — bitwise complement
* :doc:`/reference/language/structured-text/logical-operators` — the operator forms

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: XOR <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_xor.html>`_
