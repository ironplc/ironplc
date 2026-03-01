===
FOR
===

The ``FOR`` statement executes a statement list a counted number of times.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.4
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   FOR control_variable := start_expression TO end_expression [ BY step_expression ] DO
       statement_list
   END_FOR ;

If the ``BY`` clause is omitted, the step defaults to 1.

Description
-----------

The ``FOR`` loop assigns the start expression to the control variable, then
executes the statement list repeatedly. After each iteration, the control
variable is incremented by the step value. The loop terminates when the
control variable exceeds the end value (or falls below it if the step is
negative).

The control variable, start expression, end expression, and step expression
must all be integer types.

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR
           i : INT;
           total : INT := 0;
       END_VAR

       FOR i := 1 TO 10 BY 1 DO
           total := total + i;
       END_FOR;
   END_PROGRAM

See Also
--------

- :doc:`while` — pre-tested loop
- :doc:`repeat` — post-tested loop
- :doc:`exit` — break from innermost loop
