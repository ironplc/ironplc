===
MID
===

Returns a substring from the middle of a string.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.7
   * - **Support**
     - Not yet supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 15 15 15 15 30

   * - #
     - Input (IN)
     - Input (L)
     - Input (P)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``INT``
     - ``INT``
     - ``STRING``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``INT``
     - ``INT``
     - ``WSTRING``
     - Not yet supported

Description
-----------

``MID(IN, L, P)`` returns *L* characters from *IN* starting at
position *P*. Positions are 1-based: the first character is at
position 1.

Example
-------

.. code-block:: iec61131

   result := MID('Hello World', 5, 1);   (* result = 'Hello' *)
   result := MID('Hello World', 5, 7);   (* result = 'World' *)

See Also
--------

- :doc:`left` — left substring
- :doc:`right` — right substring
- :doc:`len` — string length
