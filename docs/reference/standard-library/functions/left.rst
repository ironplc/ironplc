====
LEFT
====

Returns the leftmost characters of a string.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │  LEFT   ├─ OUT
        L ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION LEFT : ANY_STRING
     VAR_INPUT
       IN : ANY_STRING;
       L  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``LEFT`` accepts ``STRING``
for *IN*; *L* is ``INT``.

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
     - Number of leftmost characters to return.

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
     - The leftmost L characters of IN. Same type as IN.

Description
-----------

Returns the leftmost *L* characters of *IN*. If *L* is greater than
or equal to the length of *IN*, the entire string is returned.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := LEFT('Hello', 3);    (* result = 'Hel' *)
   result := LEFT('Hi', 10);      (* result = 'Hi' *)

See Also
--------

* :doc:`right` — right substring
* :doc:`mid` — middle substring
* :doc:`len` — string length

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: LEFT <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/LEFT.html>`_
* `Beckhoff TwinCAT 3: LEFT <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74417163.html>`_
* `Fernhill SCADA: LEFT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-left.html>`_
