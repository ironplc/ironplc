====
FIND
====

Searches for a substring within a string.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │  FIND   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION FIND : ANY_INT
     VAR_INPUT
       IN1 : ANY_STRING;
       IN2 : ANY_STRING;
     END_VAR
   END_FUNCTION

Returns ``INT``. ``FIND`` accepts ``STRING`` for *IN1* and *IN2*. Both
inputs must share the same type.

Description
-----------

``FIND(IN1, IN2)`` returns the position of the first occurrence of
*IN2* within *IN1*. Positions are 1-based. If *IN2* is not found,
the function returns 0.

Example
-------

.. playground-with-program::
   :vars: result : INT;

   result := FIND('Hello World', 'World');   (* result = 7 *)
   result := FIND('Hello World', 'xyz');     (* result = 0 *)
   result := FIND('ABCABC', 'BC');           (* result = 2 *)

See Also
--------

* :doc:`replace` — string replacement
* :doc:`mid` — middle substring
* :doc:`len` — string length

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: FIND <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/FIND.html>`_
* `Beckhoff TwinCAT 3: FIND <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74414091.html>`_
* `Fernhill SCADA: FIND <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-find.html>`_
