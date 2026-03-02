===
SUB
===

Returns the difference of two inputs.

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

.. code-block::

   result := SUB(30, 10);   (* result = 20 *)
   result := 30 - 10;       (* result = 20, operator form *)

See Also
--------

- :doc:`add` — addition
- :doc:`mul` — multiplication
- :doc:`div` — division
