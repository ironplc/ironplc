======
INSERT
======

Inserts a string into another string at a specified position.

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
     - Input (IN1)
     - Input (IN2)
     - Input (P)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``STRING``
     - ``INT``
     - ``STRING``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``WSTRING``
     - ``INT``
     - ``WSTRING``
     - Not yet supported

Description
-----------

``INSERT(IN1, IN2, P)`` inserts *IN2* into *IN1* after position *P*.
Positions are 1-based. If *P* is 0, *IN2* is inserted before the
first character.

Example
-------

.. code-block::

   result := INSERT('Helo', 'l', 3);       (* result = 'Hello' *)
   result := INSERT('World', 'Hello ', 0); (* result = 'Hello World' *)

See Also
--------

- :doc:`delete` — string deletion
- :doc:`replace` — string replacement
- :doc:`concat` — string concatenation
