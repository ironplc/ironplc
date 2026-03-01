===
MAX
===

Returns the larger of two inputs.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.5
   * - **Support**
     - Not yet supported

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
     - Not yet supported
   * - 2
     - ``INT``
     - ``INT``
     - ``INT``
     - Not yet supported
   * - 3
     - ``DINT``
     - ``DINT``
     - ``DINT``
     - Not yet supported
   * - 4
     - ``LINT``
     - ``LINT``
     - ``LINT``
     - Not yet supported
   * - 5
     - ``USINT``
     - ``USINT``
     - ``USINT``
     - Not yet supported
   * - 6
     - ``UINT``
     - ``UINT``
     - ``UINT``
     - Not yet supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``UDINT``
     - Not yet supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``ULINT``
     - Not yet supported
   * - 9
     - ``REAL``
     - ``REAL``
     - ``REAL``
     - Not yet supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``
     - Not yet supported

Description
-----------

Returns the larger of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. code-block::

   result := MAX(10, 20);    (* result = 20 *)
   result := MAX(-5, 3);     (* result = 3 *)

See Also
--------

- :doc:`min` — minimum of two values
- :doc:`limit` — clamp to range
- :doc:`sel` — binary selection
