======
INSERT
======

Inserts a string into another string at a specified position.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤ INSERT  ├─ OUT
         P ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION INSERT : ANY_STRING
     VAR_INPUT
       IN1 : ANY_STRING;
       IN2 : ANY_STRING;
       P   : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``INSERT`` accepts ``STRING``
for *IN1* and *IN2*; *P* is ``INT``. Both string inputs must share the
same type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_STRING``
     - The base string into which IN2 is inserted.
   * - ``IN2``
     - ``ANY_STRING``
     - The string to insert.
   * - ``P``
     - ``INT``
     - Position in IN1 after which IN2 is inserted (1-based; 0 inserts at the start).

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
     - IN1 with IN2 inserted after position P. Same type as the input strings.

Description
-----------

``INSERT(IN1, IN2, P)`` inserts *IN2* into *IN1* after position *P*.
Positions are 1-based. If *P* is 0, *IN2* is inserted before the
first character.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := INSERT('Helo', 'l', 3);       (* result = 'Hello' *)
   result := INSERT('World', 'Hello ', 0); (* result = 'Hello World' *)

See Also
--------

* :doc:`delete` — string deletion
* :doc:`replace` — string replacement
* :doc:`concat` — string concatenation

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: INSERT <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/INSERT.html>`_
* `Beckhoff TwinCAT 3: INSERT <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74415627.html>`_
* `Fernhill SCADA: INSERT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-insert.html>`_
