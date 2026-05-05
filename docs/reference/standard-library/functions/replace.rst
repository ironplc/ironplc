=======
REPLACE
=======

Replaces characters in a string.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤         │
            │ REPLACE ├─ OUT
         L ─┤         │
         P ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION REPLACE : ANY_STRING
     VAR_INPUT
       IN1 : ANY_STRING;
       IN2 : ANY_STRING;
       L   : ANY_INT;
       P   : ANY_INT;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``REPLACE`` accepts ``STRING``
for *IN1* and *IN2*; *L* and *P* are ``INT``. Both string inputs must
share the same type.

Description
-----------

``REPLACE(IN1, IN2, L, P)`` replaces *L* characters in *IN1* with
*IN2* starting at position *P*. Positions are 1-based.

The replacement string *IN2* does not need to be the same length as
the portion being replaced.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := REPLACE('Hello World', 'Earth', 5, 7);  (* result = 'Hello Earth' *)
   result := REPLACE('ABCDE', 'XY', 2, 2);           (* result = 'AXYDE' *)

See Also
--------

* :doc:`insert` — string insertion
* :doc:`delete` — string deletion
* :doc:`find` — string search

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: REPLACE <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/REPLACE.html>`_
* `Beckhoff TwinCAT 3: REPLACE <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74421771.html>`_
* `Fernhill SCADA: REPLACE <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-replace.html>`_
