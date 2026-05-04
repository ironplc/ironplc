===
LEN
===

Returns the length of a string.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   LEN   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION LEN : ANY_INT
     VAR_INPUT
       IN : ANY_STRING;
     END_VAR
   END_FUNCTION

Returns ``INT``. ``LEN`` accepts ``STRING``.

Description
-----------

Returns the number of characters in *IN*. For an empty string, the
result is 0.

Example
-------

.. playground-with-program::
   :vars: result : INT;

   result := LEN('Hello');    (* result = 5 *)
   result := LEN('');         (* result = 0 *)

See Also
--------

* :doc:`left` — left substring
* :doc:`right` — right substring
* :doc:`mid` — middle substring

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: LEN <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/LEN.html>`_
* `Beckhoff TwinCAT 3: LEN <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74418699.html>`_
* `Fernhill SCADA: LEN <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-len.html>`_
