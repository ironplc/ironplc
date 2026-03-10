===
MIN
===

Returns the smaller of two inputs.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.5
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 20 20 20 30

   * - #
     - Input (IN1)
     - Input (IN2)
     - Return Type
     - Support
   * - 1
     - ``SINT``
     - ``SINT``
     - ``SINT``
     - Supported
   * - 2
     - ``INT``
     - ``INT``
     - ``INT``
     - Supported
   * - 3
     - ``DINT``
     - ``DINT``
     - ``DINT``
     - Supported
   * - 4
     - ``LINT``
     - ``LINT``
     - ``LINT``
     - Supported
   * - 5
     - ``USINT``
     - ``USINT``
     - ``USINT``
     - Supported
   * - 6
     - ``UINT``
     - ``UINT``
     - ``UINT``
     - Supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``UDINT``
     - Supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``ULINT``
     - Supported
   * - 9
     - ``REAL``
     - ``REAL``
     - ``REAL``
     - Supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``
     - Supported

Description
-----------

Returns the smaller of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MIN(10, 20);    (* result = 10 *)
   result := MIN(-5, 3);     (* result = -5 *)

See Also
--------

- :doc:`max` — maximum of two values
- :doc:`limit` — clamp to range
- :doc:`sel` — binary selection
