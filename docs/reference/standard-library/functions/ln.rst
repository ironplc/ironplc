==
LN
==

Returns the natural logarithm of a numeric input.

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

Returns the natural logarithm (base *e*) of *IN*. The input must be
positive; the result of ``LN`` applied to zero or a negative value
is undefined.

Example
-------

.. code-block:: iec61131

   result := LN(REAL#2.718282);  (* result ~ 1.0 *)
   value := LN(LREAL#1.0);      (* value = 0.0 *)

See Also
--------

- :doc:`log` — base-10 logarithm
- :doc:`exp` — natural exponential
