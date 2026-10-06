========
CONTINUE
========

The ``CONTINUE`` statement goes on with the next iteration of the innermost
enclosing loop.

.. include:: ../../../includes/requires-edition3.rst

``CONTINUE`` can also be enabled without full Edition 3.

.. |flag| replace:: ``--allow-continue``
.. include:: /includes/enabled-by-flag.rst

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   CONTINUE ;

Description
-----------

``CONTINUE`` skips the rest of the body of the innermost ``FOR``, ``WHILE``,
or ``REPEAT`` loop. The loop then goes on as it does at the end of its body:

- ``FOR`` adds the step to the control variable and tests the end value;
- ``WHILE`` tests its condition;
- ``REPEAT`` tests its ``UNTIL`` condition.

If ``CONTINUE`` appears inside nested loops, only the innermost loop goes
on with its next iteration. ``CONTINUE`` outside a loop is an error
(:doc:`P4065 </reference/compiler/problems/P4065>`).

Example
-------

.. playground::
   :dialect: iec61131-3-ed3

   PROGRAM main
       VAR
           i : INT;
           odd_sum : INT := 0;
       END_VAR

       FOR i := 1 TO 10 DO
           IF i MOD 2 = 0 THEN
               CONTINUE;
           END_IF;
           odd_sum := odd_sum + i;
       END_FOR;
   END_PROGRAM

See Also
--------

- :doc:`exit` — break from the innermost loop
- :doc:`for` — counted loop
- :doc:`while` — pre-tested loop
- :doc:`repeat` — post-tested loop
