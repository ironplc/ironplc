===
MID
===

Returns a substring from the middle of a string.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
        L ─┤   MID   ├─ OUT
        P ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION MID : ANY_STRING
     VAR_INPUT
       IN : ANY_STRING;
       L  : ANY_INT;
       P  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``MID`` accepts ``STRING``
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
     - Number of characters to extract.
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
     - L characters from IN starting at position P. Same type as IN.

Description
-----------

``MID(IN, L, P)`` returns *L* characters from *IN* starting at
position *P*. Positions are 1-based: the first character is at
position 1.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := MID('Hello World', 5, 1);   (* result = 'Hello' *)
   result := MID('Hello World', 5, 7);   (* result = 'World' *)

See Also
--------

* :doc:`left` — left substring
* :doc:`right` — right substring
* :doc:`len` — string length

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: MID <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/MID.html>`_
* `Beckhoff TwinCAT 3: MID <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74420235.html>`_
* `Fernhill SCADA: MID <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-mid.html>`_
