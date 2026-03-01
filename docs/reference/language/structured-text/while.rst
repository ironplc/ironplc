=====
WHILE
=====

The ``WHILE`` statement repeatedly executes a statement list as long as a
boolean expression is ``TRUE``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.4
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   WHILE expression DO
       statement_list
   END_WHILE ;

Description
-----------

The ``WHILE`` loop evaluates the boolean expression before each iteration. If
the expression is ``TRUE``, the statement list executes and the expression is
evaluated again. If the expression is ``FALSE`` on the first evaluation, the
statement list never executes.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           count : INT := 10;
           total : INT := 0;
       END_VAR

       WHILE count > 0 DO
           total := total + count;
           count := count - 1;
       END_WHILE;
   END_PROGRAM

See Also
--------

- :doc:`for` — counted loop
- :doc:`repeat` — post-tested loop
- :doc:`exit` — break from innermost loop
