====
MOVE
====

Copies the input value to the output (assignment).

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.4
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
     - ``USINT``
     - ``USINT``
     - Supported
   * - 6
     - ``UINT``
     - ``UINT``
     - Supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - Supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - Supported
   * - 9
     - ``REAL``
     - ``REAL``
     - Supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - Supported

Description
-----------

Copies the value of *IN* to the output. ``MOVE`` is the functional form
of the ``:=`` assignment operator. It is useful when an explicit function
call is preferred over the assignment syntax, for example as an argument
to other functions.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MOVE(42);       (* result = 42 *)

See Also
--------

- :doc:`sel` — binary selection
- :doc:`limit` — clamp to range
