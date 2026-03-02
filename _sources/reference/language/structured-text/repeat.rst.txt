======
REPEAT
======

The ``REPEAT`` statement executes a statement list and then evaluates a
boolean condition to determine whether to repeat.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.4
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   REPEAT
       statement_list
   UNTIL expression
   END_REPEAT ;

Description
-----------

The ``REPEAT`` loop executes the statement list at least once. After each
iteration, the boolean expression is evaluated. If the expression is ``TRUE``,
the loop terminates. If it is ``FALSE``, the statement list executes again.

This is the opposite of a ``WHILE`` loop: ``WHILE`` continues while the
condition is true, whereas ``REPEAT`` continues until the condition becomes
true.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           count : INT := 0;
       END_VAR

       REPEAT
           count := count + 1;
       UNTIL count >= 10
       END_REPEAT;
   END_PROGRAM

See Also
--------

- :doc:`while` — pre-tested loop
- :doc:`for` — counted loop
- :doc:`exit` — break from innermost loop
