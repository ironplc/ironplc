====
SQRT
====

Returns the square root of a numeric input.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.2
   * - **Support**
     - Not yet supported

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
     - ``REAL``
     - ``REAL``
     - Not yet supported
   * - 2
     - ``LREAL``
     - ``LREAL``
     - Not yet supported

Description
-----------

Returns the square root of *IN*. The input must be non-negative;
the result of ``SQRT`` applied to a negative value is undefined.

Example
-------

.. code-block:: iec61131

   result := SQRT(REAL#9.0);    (* result = 3.0 *)
   value := SQRT(LREAL#2.0);   (* value = 1.41421356... *)

See Also
--------

- :doc:`abs` — absolute value
- :doc:`expt` — exponentiation
- :doc:`exp` — natural exponential
