===
MUL
===

Returns the product of two or more inputs.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.3
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 20 20 20

   * - #
     - Input (IN1)
     - Input (IN2)
     - Return Type
   * - 1
     - ``SINT``
     - ``SINT``
     - ``SINT``
   * - 2
     - ``INT``
     - ``INT``
     - ``INT``
   * - 3
     - ``DINT``
     - ``DINT``
     - ``DINT``
   * - 4
     - ``LINT``
     - ``LINT``
     - ``LINT``
   * - 5
     - ``USINT``
     - ``USINT``
     - ``USINT``
   * - 6
     - ``UINT``
     - ``UINT``
     - ``UINT``
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``UDINT``
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``ULINT``
   * - 9
     - ``REAL``
     - ``REAL``
     - ``REAL``
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``

Description
-----------

Returns *IN1* multiplied by *IN2*. ``MUL(a, b)`` is the functional
form of the ``*`` operator: ``a * b``. Both forms are equivalent.

For integer types, overflow behavior wraps around (modular arithmetic).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MUL(6, 7);   (* result = 42 *)
   result := 6 * 7;       (* result = 42, operator form *)

See Also
--------

- :doc:`add` — addition
- :doc:`div` — division
- :doc:`mod` — modulo
