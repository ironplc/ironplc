======
CONCAT
======

Concatenates two strings.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │ CONCAT  ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION CONCAT : ANY_STRING
     VAR_INPUT
       IN1 : ANY_STRING;
       IN2 : ANY_STRING;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``CONCAT`` accepts ``STRING``.
Both inputs must share the same type.

Description
-----------

Returns a new string formed by appending *IN2* to the end of *IN1*.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := CONCAT('Hello', ' World');    (* result = 'Hello World' *)
   result := CONCAT('A', 'B');             (* result = 'AB' *)

See Also
--------

* :doc:`insert` — string insertion
* :doc:`len` — string length
* :doc:`left` — left substring

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: CONCAT <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/CONCAT.html>`_
* `Beckhoff TwinCAT 3: CONCAT <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74411019.html>`_
* `Fernhill SCADA: CONCAT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-concat.html>`_
