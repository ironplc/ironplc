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

Description
-----------

Returns the remainder of *IN1* divided by *IN2*. ``MOD(a, b)`` is the
functional form of the ``MOD`` operator: ``a MOD b``. Both forms are
equivalent.

The result has the same sign as *IN1*. The ``MOD`` function is defined
only for integer types. Division by zero causes a runtime fault.

Example
-------

.. code-block::

   result := MOD(7, 3);    (* result = 1 *)
   result := 7 MOD 3;      (* result = 1, operator form *)
   result := -7 MOD 3;     (* result = -1 *)

See Also
--------

- :doc:`div` — division
- :doc:`mul` — multiplication
