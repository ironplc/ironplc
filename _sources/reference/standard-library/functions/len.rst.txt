===
LEN
===

Returns the length of a string.

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
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``INT``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``INT``
     - Not yet supported

Description
-----------

Returns the number of characters in *IN*. For an empty string, the
result is 0.

Example
-------

.. code-block::

   result := LEN('Hello');    (* result = 5 *)
   result := LEN('');         (* result = 0 *)

See Also
--------

- :doc:`left` — left substring
- :doc:`right` — right substring
- :doc:`mid` — middle substring
