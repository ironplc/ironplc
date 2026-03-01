===
LOG
===

Returns the base-10 logarithm of a numeric input.

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

Returns the common logarithm (base 10) of *IN*. The input must be
positive; the result of ``LOG`` applied to zero or a negative value
is undefined.

Example
-------

.. code-block:: iec61131

   result := LOG(REAL#100.0);  (* result = 2.0 *)
   value := LOG(LREAL#1000.0); (* value = 3.0 *)

See Also
--------

- :doc:`ln` — natural logarithm
- :doc:`exp` — natural exponential
