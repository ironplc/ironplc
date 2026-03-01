====
EXPT
====

Returns the result of raising a base to an exponent.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.2
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
     - ``REAL``
     - ``REAL``
     - ``REAL``
     - Not yet supported
   * - 6
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``
     - Not yet supported

Description
-----------

Returns *IN1* raised to the power *IN2*. ``EXPT(a, b)`` computes
*a*\ :sup:`b`. For integer types, the exponent must be non-negative.
The operator form is ``**``.

Example
-------

.. code-block:: iec61131

   result := EXPT(2, 10);       (* result = 1024 *)
   value := 3 ** 4;             (* value = 81, operator form *)

See Also
--------

- :doc:`exp` — natural exponential (*e*\ :sup:`x`)
- :doc:`sqrt` — square root
- :doc:`abs` — absolute value
