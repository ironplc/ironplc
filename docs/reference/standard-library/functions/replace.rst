=======
REPLACE
=======

Replaces characters in a string.

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
   :widths: 10 12 12 12 12 12 30

   * - #
     - Input (IN1)
     - Input (IN2)
     - Input (L)
     - Input (P)
     - Return Type
     - Support
   * - 1
     - ``STRING``
     - ``STRING``
     - ``INT``
     - ``INT``
     - ``STRING``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``WSTRING``
     - ``INT``
     - ``INT``
     - ``WSTRING``
     - Not yet supported

Description
-----------

``REPLACE(IN1, IN2, L, P)`` replaces *L* characters in *IN1* with
*IN2* starting at position *P*. Positions are 1-based.

The replacement string *IN2* does not need to be the same length as
the portion being replaced.

Example
-------

.. code-block::

   result := REPLACE('Hello World', 'Earth', 5, 7);  (* result = 'Hello Earth' *)
   result := REPLACE('ABCDE', 'XY', 2, 2);           (* result = 'AXYDE' *)

See Also
--------

- :doc:`insert` — string insertion
- :doc:`delete` — string deletion
- :doc:`find` — string search
