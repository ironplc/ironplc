=====
RIGHT
=====

Returns the rightmost characters of a string.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤         │
           │  RIGHT  ├─ OUT
        L ─┤         │
           └─────────┘

.. code-block:: text

   FUNCTION RIGHT : ANY_STRING
     VAR_INPUT
       IN : ANY_STRING;
       L  : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN*. ``RIGHT`` accepts ``STRING``
for *IN*; *L* is ``INT``.

Description
-----------

Returns the rightmost *L* characters of *IN*. If *L* is greater than
or equal to the length of *IN*, the entire string is returned.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := RIGHT('Hello', 3);    (* result = 'llo' *)
   result := RIGHT('Hi', 10);      (* result = 'Hi' *)

See Also
--------

* :doc:`left` — left substring
* :doc:`mid` — middle substring
* :doc:`len` — string length

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: RIGHT <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/RIGHT.html>`_
* `Beckhoff TwinCAT 3: RIGHT <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74423307.html>`_
* `Fernhill SCADA: RIGHT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-right.html>`_
