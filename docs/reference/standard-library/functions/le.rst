==
LE
==

Returns TRUE if the first input is less than or equal to the second.

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
   :widths: 10 20 20 20

   * - #
     - Input (IN1)
     - Input (IN2)
     - Return Type
   * - 1
     - ``SINT``
     - ``SINT``
     - ``BOOL``
   * - 2
     - ``INT``
     - ``INT``
     - ``BOOL``
   * - 3
     - ``DINT``
     - ``DINT``
     - ``BOOL``
   * - 4
     - ``LINT``
     - ``LINT``
     - ``BOOL``
   * - 5
     - ``USINT``
     - ``USINT``
     - ``BOOL``
   * - 6
     - ``UINT``
     - ``UINT``
     - ``BOOL``
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``BOOL``
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``BOOL``
   * - 9
     - ``REAL``
     - ``REAL``
     - ``BOOL``
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``BOOL``

Description
-----------

Returns ``TRUE`` if *IN1* is less than or equal to *IN2*, ``FALSE``
otherwise. ``LE(a, b)`` is the functional form of the ``<=`` operator:
``a <= b``. Both forms are equivalent.

Example
-------

.. playground-with-program::
   :vars: result : BOOL;

   result := LE(5, 10);    (* result = TRUE *)
   result := 5 <= 10;      (* result = TRUE, operator form *)
   result := 5 <= 5;       (* result = TRUE *)

See Also
--------

- :doc:`lt` — less than
- :doc:`ge` — greater than or equal
- :doc:`eq` — equal
