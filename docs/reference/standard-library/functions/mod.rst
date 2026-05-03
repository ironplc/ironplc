===
MOD
===

Returns the remainder after integer division.

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

Description
-----------

Returns the remainder of *IN1* divided by *IN2*. ``MOD(a, b)`` is the
functional form of the ``MOD`` operator: ``a MOD b``. Both forms are
equivalent.

The result has the same sign as *IN1*. IEC 61131-3 defines the ``MOD``
function only for integer types. Division by zero causes a runtime fault.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MOD(7, 3);    (* result = 1 *)
   result := 7 MOD 3;      (* result = 1, operator form *)
   result := -7 MOD 3;     (* result = -1 *)

See Also
--------

- :doc:`div` — division
- :doc:`mul` — multiplication
