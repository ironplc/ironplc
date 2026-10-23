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
must all be integer types. A control variable of any other type, such as a
``REAL`` or a ``WORD``, is an error
(:doc:`P4091 </reference/compiler/problems/P4091>`). A real literal such as
``1.5`` as the start expression, end expression or step expression of an
integer loop is an error too
(:doc:`P4094 </reference/compiler/problems/P4094>`).

The step must not be zero: a step of zero never moves the control variable,
so the loop never ends. A step written as ``0``, as an expression that folds
to zero, or as the name of a constant whose value is zero is an error
(:doc:`P4093 </reference/compiler/problems/P4093>`).

IEC 61131-3 allows any integer expression as the step, but IronPLC compiles
only a literal step, or an expression of literals such as ``2 * 3``. A step
read from a variable, or one that names a constant, passes analysis but is
not yet implemented and reports
:doc:`P9999 </reference/compiler/problems/P9999>` when compiled.

Example
-------

.. playground::

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
- :doc:`continue` — next iteration of innermost loop
