===
SUB
===

Returns the difference of two inputs.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.3
   * - **Support**
     - Supported (INT)

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
     - Supported
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
   * - 11
     - ``TIME``
     - ``TIME``
     - ``TIME``
     - Not yet supported

Description
-----------

Returns *IN1* minus *IN2*. ``SUB(a, b)`` is the functional form of the
``-`` operator: ``a - b``. Both forms are equivalent.

For integer types, underflow behavior wraps around (modular arithmetic).

Example
-------

.. code-block:: iec61131

   result := SUB(30, 10);   (* result = 20 *)
   result := 30 - 10;       (* result = 20, operator form *)

See Also
--------

- :doc:`add` — addition
- :doc:`mul` — multiplication
- :doc:`div` — division
