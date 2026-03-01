==
GT
==

Returns TRUE if the first input is greater than the second.

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

Returns ``TRUE`` if *IN1* is strictly greater than *IN2*, ``FALSE``
otherwise. ``GT(a, b)`` is the functional form of the ``>`` operator:
``a > b``. Both forms are equivalent.

Example
-------

.. code-block::

   result := GT(10, 5);    (* result = TRUE *)
   result := 10 > 5;       (* result = TRUE, operator form *)
   result := 5 > 5;        (* result = FALSE *)

See Also
--------

- :doc:`ge` — greater than or equal
- :doc:`lt` — less than
- :doc:`eq` — equal
