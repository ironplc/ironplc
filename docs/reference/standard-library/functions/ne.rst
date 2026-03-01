==
NE
==

Returns TRUE if two inputs are not equal.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.4
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
     - ``BOOL``
     - Not yet supported
   * - 2
     - ``INT``
     - ``INT``
     - ``BOOL``
     - Supported
   * - 3
     - ``DINT``
     - ``DINT``
     - ``BOOL``
     - Not yet supported
   * - 4
     - ``LINT``
     - ``LINT``
     - ``BOOL``
     - Not yet supported
   * - 5
     - ``USINT``
     - ``USINT``
     - ``BOOL``
     - Not yet supported
   * - 6
     - ``UINT``
     - ``UINT``
     - ``BOOL``
     - Not yet supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``BOOL``
     - Not yet supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``BOOL``
     - Not yet supported
   * - 9
     - ``REAL``
     - ``REAL``
     - ``BOOL``
     - Not yet supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``BOOL``
     - Not yet supported

Description
-----------

Returns ``TRUE`` if *IN1* is not equal to *IN2*, ``FALSE`` otherwise.
``NE(a, b)`` is the functional form of the ``<>`` operator: ``a <> b``.
Both forms are equivalent.

For ``REAL`` and ``LREAL`` types, inequality comparison is subject to
floating-point precision limitations.

Example
-------

.. code-block:: iec61131

   result := NE(5, 10);    (* result = TRUE *)
   result := 5 <> 10;      (* result = TRUE, operator form *)
   result := 5 <> 5;       (* result = FALSE *)

See Also
--------

- :doc:`eq` — equal
- :doc:`gt` — greater than
- :doc:`lt` — less than
