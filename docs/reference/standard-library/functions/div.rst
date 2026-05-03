===
DIV
===

Returns the quotient of two inputs.

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

- :doc:`mul` — multiplication
- :doc:`mod` — modulo
- :doc:`sub` — subtraction
