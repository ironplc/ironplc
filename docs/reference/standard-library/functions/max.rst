===
MAX
===

Returns the larger of two inputs.

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

Returns the larger of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. playground::
   :vars: result : DINT;

   result := MAX(10, 20);    (* result = 20 *)
   result := MAX(-5, 3);     (* result = 3 *)

See Also
--------

- :doc:`min` — minimum of two values
- :doc:`limit` — clamp to range
- :doc:`sel` — binary selection
