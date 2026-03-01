===
EXP
===

Returns the natural exponential of a numeric input.

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

Returns *e* raised to the power of *IN*, where *e* is Euler's number
(approximately 2.71828). This is the inverse of the :doc:`ln` function.

Example
-------

.. code-block:: iec61131

   result := EXP(REAL#1.0);   (* result ~ 2.718282 *)
   value := EXP(LREAL#0.0);   (* value = 1.0 *)

See Also
--------

- :doc:`ln` — natural logarithm
- :doc:`expt` — exponentiation
