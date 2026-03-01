======
DELETE
======

Deletes characters from a string.

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

``DELETE(IN, L, P)`` deletes *L* characters from *IN* starting at
position *P*. Positions are 1-based.

Example
-------

.. code-block::

   result := DELETE('Hello World', 6, 6);   (* result = 'Hello' *)
   result := DELETE('ABCDE', 2, 2);         (* result = 'ADE' *)

See Also
--------

- :doc:`insert` — string insertion
- :doc:`replace` — string replacement
- :doc:`mid` — middle substring
