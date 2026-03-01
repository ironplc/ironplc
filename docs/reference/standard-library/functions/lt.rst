==
LT
==

Returns TRUE if the first input is less than the second.

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
     - Supported
   * - 2
     - ``INT``
     - ``INT``
     - ``BOOL``
     - Supported
   * - 3
     - ``DINT``
     - ``DINT``
     - ``BOOL``
     - Supported
   * - 4
     - ``LINT``
     - ``LINT``
     - ``BOOL``
     - Supported
   * - 5
     - ``USINT``
     - ``USINT``
     - ``BOOL``
     - Supported
   * - 6
     - ``UINT``
     - ``UINT``
     - ``BOOL``
     - Supported
   * - 7
     - ``UDINT``
     - ``UDINT``
     - ``BOOL``
     - Supported
   * - 8
     - ``ULINT``
     - ``ULINT``
     - ``BOOL``
     - Supported
   * - 9
     - ``REAL``
     - ``REAL``
     - ``BOOL``
     - Supported
   * - 10
     - ``LREAL``
     - ``LREAL``
     - ``BOOL``
     - Supported

Description
-----------

Returns ``TRUE`` if *IN1* is strictly less than *IN2*, ``FALSE``
otherwise. ``LT(a, b)`` is the functional form of the ``<`` operator:
``a < b``. Both forms are equivalent.

Example
-------

.. code-block::

   result := LT(5, 10);    (* result = TRUE *)
   result := 5 < 10;       (* result = TRUE, operator form *)
   result := 5 < 5;        (* result = FALSE *)

See Also
--------

- :doc:`le` — less than or equal
- :doc:`gt` — greater than
- :doc:`eq` — equal
