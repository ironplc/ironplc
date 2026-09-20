===
NOT
===

Returns the bitwise complement of its input.

Signature
---------

.. code-block:: text

            ┌─────────┐
        IN ─┤   NOT   ├─ OUT
            └─────────┘

.. code-block:: text

   FUNCTION NOT : ANY_BIT
     VAR_INPUT
       IN : ANY_BIT;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``NOT`` accepts ``BOOL``, ``BYTE``,
``WORD``, ``DWORD``, ``LWORD``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_BIT``
     - The operand.

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
     - Every bit of IN inverted. Same type as IN.

Description
-----------

On a ``BOOL`` input, returns ``TRUE`` when the input is ``FALSE`` and
``FALSE`` when it is ``TRUE``. On ``BYTE``, ``WORD``, ``DWORD`` and ``LWORD``
inputs, every bit of the result is the inverse of the same bit of the input.
``NOT(IN := a)`` is the functional form of the ``NOT`` operator: ``NOT a``.
Both forms are equivalent, and ``NOT(a)`` is read as the operator applied to
``(a)``.

Example
-------

.. playground-with-program::
   :vars: result : WORD;

   result := NOT(IN := WORD#16#F0F0);   (* result = 16#0F0F *)
   result := NOT WORD#16#F0F0;          (* operator form *)

See Also
--------

* :doc:`and` — bitwise AND
* :doc:`or` — bitwise OR
* :doc:`xor` — bitwise exclusive OR
* :doc:`/reference/language/structured-text/logical-operators` — the operator forms

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: NOT <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_not.html>`_
