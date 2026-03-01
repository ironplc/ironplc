=====
RIGHT
=====

Returns the rightmost characters of a string.

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
   :widths: 10 20 20 20 30

   * - #
     - Input (IN)
     - Input (L)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``INT``
     - ``STRING``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``INT``
     - ``WSTRING``
     - Not yet supported

Description
-----------

Returns the rightmost *L* characters of *IN*. If *L* is greater than
or equal to the length of *IN*, the entire string is returned.

Example
-------

.. code-block:: iec61131

   result := RIGHT('Hello', 3);    (* result = 'llo' *)
   result := RIGHT('Hi', 10);      (* result = 'Hi' *)

See Also
--------

- :doc:`left` — left substring
- :doc:`mid` — middle substring
- :doc:`len` — string length
