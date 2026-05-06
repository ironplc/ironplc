======
DELETE
======

Deletes characters from a string.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
        L ─┤ DELETE  ├─ OUT
        P ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION DELETE : ANY_STRING
     VAR_INPUT
       IN : ANY_STRING;
       L  : ANY_INT;
       P  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``DELETE`` accepts ``STRING``
for *IN*; *L* and *P* are ``INT``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_STRING``
     - The source string.
   * - ``L``
     - ``INT``
     - Number of characters to delete.
   * - ``P``
     - ``INT``
     - Starting position (1-based).

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_STRING``
     - IN with L characters removed starting at position P. Same type as IN.

Description
-----------

``DELETE(IN, L, P)`` deletes *L* characters from *IN* starting at
position *P*. Positions are 1-based.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := DELETE('Hello World', 6, 6);   (* result = 'Hello' *)
   result := DELETE('ABCDE', 2, 2);         (* result = 'ADE' *)

See Also
--------

* :doc:`insert` — string insertion
* :doc:`replace` — string replacement
* :doc:`mid` — middle substring

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: DELETE <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/DELETE.html>`_
* `Beckhoff TwinCAT 3: DELETE <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74412555.html>`_
* `Fernhill SCADA: DELETE <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-delete.html>`_
