=====
LIMIT
=====

Clamps a value to a specified range.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.5
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 15 15 15 15 30

   * - #
     - Input (MN)
     - Input (IN)
     - Input (MX)
     - Return Type
     - Support
   * - 1
     - ``SINT``
     - ``SINT``
     - ``SINT``
     - ``SINT``
     - Supported
   * - 2
     - ``INT``
     - ``INT``
     - ``INT``
     - ``INT``
     - Supported
   * - 3
     - ``DINT``
     - ``DINT``
     - ``DINT``
     - ``DINT``
     - Supported
   * - 4
     - ``LINT``
     - ``LINT``
     - ``LINT``
     - ``LINT``
     - Supported
   * - 5
     - ``USINT``
     - ``USINT``
     - ``USINT``
     - ``USINT``
     - Supported
   * - 6
     - ``UINT``
     - ``UINT``
     - ``UINT``
     - ``UINT``
     - Supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``UDINT``
     - ``UDINT``
     - Supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``ULINT``
     - ``ULINT``
     - Supported
   * - 9
     - ``REAL``
     - ``REAL``
     - ``REAL``
     - ``REAL``
     - Supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``
     - ``LREAL``
     - Supported

Description
-----------

``LIMIT(MN, IN, MX)`` clamps *IN* to the range [*MN*, *MX*]. The
function returns:

- *MN* if *IN* < *MN*
- *MX* if *IN* > *MX*
- *IN* otherwise

The behavior is undefined if *MN* > *MX*.

Example
-------

.. playground::
   :vars: result : DINT;

   result := LIMIT(0, 50, 100);    (* result = 50 *)
   result := LIMIT(0, -10, 100);   (* result = 0 *)
   result := LIMIT(0, 200, 100);   (* result = 100 *)

See Also
--------

- :doc:`max` — maximum of two values
- :doc:`min` — minimum of two values
- :doc:`sel` — binary selection
