==
IF
==

The ``IF`` statement executes a block of statements conditionally based on
boolean expressions.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.3
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   IF expression THEN
       statement_list
   { ELSIF expression THEN
       statement_list }
   [ ELSE
       statement_list ]
   END_IF ;

The ``ELSIF`` and ``ELSE`` clauses are optional. Multiple ``ELSIF`` clauses
may appear.

Description
-----------

The ``IF`` statement evaluates the boolean expression after ``IF``. If it is
``TRUE``, the corresponding statement list executes. Otherwise, each ``ELSIF``
expression is evaluated in order. If none are ``TRUE`` and an ``ELSE`` clause
is present, its statement list executes.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           temperature : INT := 25;
           fan_speed : INT;
       END_VAR

       IF temperature > 80 THEN
           fan_speed := 3;
       ELSIF temperature > 60 THEN
           fan_speed := 2;
       ELSIF temperature > 40 THEN
           fan_speed := 1;
       ELSE
           fan_speed := 0;
       END_IF;
   END_PROGRAM

See Also
--------

- :doc:`case` — multi-way selection by integer value
