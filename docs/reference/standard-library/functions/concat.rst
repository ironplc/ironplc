======
CONCAT
======

Concatenates two strings.

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
     - ``STRING``
     - Not yet supported
   * - 2
     - ``WSTRING``
     - ``WSTRING``
     - ``WSTRING``
     - Not yet supported

Description
-----------

Returns a new string formed by appending *IN2* to the end of *IN1*.

Example
-------

.. code-block::

   result := CONCAT('Hello', ' World');    (* result = 'Hello World' *)
   result := CONCAT('A', 'B');             (* result = 'AB' *)

See Also
--------

- :doc:`insert` — string insertion
- :doc:`len` — string length
- :doc:`left` — left substring
