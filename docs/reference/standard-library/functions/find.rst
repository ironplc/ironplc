====
FIND
====

Searches for a substring within a string.

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
     - Input (IN1)
     - Input (IN2)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``STRING``
     - ``INT``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``WSTRING``
     - ``INT``
     - Not yet supported

Description
-----------

``FIND(IN1, IN2)`` returns the position of the first occurrence of
*IN2* within *IN1*. Positions are 1-based. If *IN2* is not found,
the function returns 0.

Example
-------

.. code-block::

   result := FIND('Hello World', 'World');   (* result = 7 *)
   result := FIND('Hello World', 'xyz');     (* result = 0 *)
   result := FIND('ABCABC', 'BC');           (* result = 2 *)

See Also
--------

- :doc:`replace` — string replacement
- :doc:`mid` — middle substring
- :doc:`len` — string length
