====
CASE
====

The ``CASE`` statement selects one of several statement groups based on the
value of an integer expression.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.3
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   CASE expression OF
       case_value { ',' case_value } ':' statement_list
       { case_value { ',' case_value } ':' statement_list }
   [ ELSE
       statement_list ]
   END_CASE ;

Each ``case_value`` is an integer literal or a subrange (``low .. high``).
Multiple values can be listed separated by commas.

Description
-----------

The ``CASE`` statement evaluates the expression and compares it against each
case value in order. When a match is found, the corresponding statement list
executes and control passes to the statement after ``END_CASE``. If no match
is found and an ``ELSE`` clause is present, its statement list executes.

Unlike C-style ``switch`` statements, there is no fall-through between cases.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           command : INT := 2;
           motor_on : BOOL;
           alarm : BOOL;
       END_VAR

       CASE command OF
           0:
               motor_on := FALSE;
               alarm := FALSE;
           1:
               motor_on := TRUE;
           2, 3:
               motor_on := TRUE;
               alarm := TRUE;
       ELSE
           alarm := TRUE;
       END_CASE;
   END_PROGRAM

See Also
--------

- :doc:`if` — conditional branching
