===
ABS
===

Returns the absolute value of a numeric input.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.2
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``SINT``
     - ``SINT``
     - Supported
   * - 2
     - ``INT``
     - ``INT``
     - Supported
   * - 3
     - ``DINT``
     - ``DINT``
     - Supported
   * - 4
     - ``LINT``
     - ``LINT``
     - Supported
   * - 5
     - ``REAL``
     - ``REAL``
     - Supported
   * - 6
     - ``LREAL``
     - ``LREAL``
     - Supported

Description
-----------

Returns the absolute value of *IN*. For signed integer types, the result
of ``ABS`` applied to the most negative value is undefined because
the positive value cannot be represented.

Example
-------

.. playground-with-program::
   :vars: result : DINT; value : REAL;

   result := ABS(-42);    (* result = 42 *)
   value := ABS(REAL#-3.14);  (* value = 3.14 *)

See Also
--------

- :doc:`sqrt` — square root
- :doc:`expt` — exponentiation
