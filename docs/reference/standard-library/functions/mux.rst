===
MUX
===

Multiplexer — selects one of several inputs by index.

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
   :widths: 10 15 15 15 15 30

   * - #
     - Input (K)
     - Input (IN0)
     - Input (IN1, ...)
     - Return Type
     - Support
   * - 1
     - ``INT``
     - *ANY*
     - *ANY*
     - *ANY*
     - Not yet supported

Description
-----------

``MUX(K, IN0, IN1, ...)`` returns the input selected by the zero-based
index *K*. The number of inputs is variable, and all inputs must be
the same type.

- If *K* = 0, returns *IN0*
- If *K* = 1, returns *IN1*
- And so on

The behavior is undefined if *K* is negative or greater than or equal
to the number of inputs.

This function is polymorphic: it works with any data type for the
selected inputs.

Example
-------

.. code-block:: iec61131

   result := MUX(0, 10, 20, 30);    (* result = 10 *)
   result := MUX(2, 10, 20, 30);    (* result = 30 *)

See Also
--------

- :doc:`sel` — binary selection (two inputs)
- :doc:`limit` — clamp to range
