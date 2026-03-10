=====
TRUNC
=====

Truncates a real (floating-point) value toward zero, removing the
fractional part and returning an integer.

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
     - ``REAL``
     - ``SINT``
     - Supported
   * - 2
     - ``REAL``
     - ``INT``
     - Supported
   * - 3
     - ``REAL``
     - ``DINT``
     - Supported
   * - 4
     - ``REAL``
     - ``LINT``
     - Supported
   * - 5
     - ``LREAL``
     - ``SINT``
     - Supported
   * - 6
     - ``LREAL``
     - ``INT``
     - Supported
   * - 7
     - ``LREAL``
     - ``DINT``
     - Supported
   * - 8
     - ``LREAL``
     - ``LINT``
     - Supported

Description
-----------

``TRUNC`` removes the fractional part of a real number, truncating toward
zero. This means positive values are rounded down and negative values
are rounded up (toward zero).

- ``TRUNC(3.7)`` returns ``3``
- ``TRUNC(-3.7)`` returns ``-3``
- ``TRUNC(0.9)`` returns ``0``

The return type is determined by the variable being assigned to.

Example
-------

.. playground-with-program::
   :vars: result : DINT; neg_result : DINT;

   result := TRUNC(REAL#3.7);       (* result = 3 *)
   neg_result := TRUNC(REAL#-3.7);  (* neg_result = -3 *)

See Also
--------

- :doc:`type-conversions` — explicit type conversion functions
- :doc:`abs` — absolute value
