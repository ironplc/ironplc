==============
FUNCTION_BLOCK
==============

A function block is a stateful callable unit with input and output
parameters. Function block instances retain their state between calls.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2
   * - **Support**
     - Partial

Syntax
------

.. code-block:: bnf

   FUNCTION_BLOCK fb_name
       variable_declarations
       statement_list
   END_FUNCTION_BLOCK

Example
-------

.. code-block::

   FUNCTION_BLOCK Counter
       VAR_INPUT
           reset : BOOL;
       END_VAR
       VAR_OUTPUT
           count : INT;
       END_VAR
       VAR
           internal : INT := 0;
       END_VAR

       IF reset THEN
           internal := 0;
       ELSE
           internal := internal + 1;
       END_IF;
       count := internal;
   END_FUNCTION_BLOCK

Using a Function Block
----------------------

Function blocks must be instantiated as variables before use:

.. code-block::

   PROGRAM main
       VAR
           my_counter : Counter;
           value : INT;
       END_VAR

       my_counter(reset := FALSE);
       value := my_counter.count;
   END_PROGRAM

Outputs are accessed using dot notation on the instance.

.. _function-block-arguments:

Arguments
---------

A call passes its inputs by name (``step := 2``) or by position, and reads its
outputs with ``=>``:

- **Positional inputs** go to the ``VAR_INPUT`` parameters in the order the
  function block declares them, and a positional call supplies every one
  (:doc:`/reference/compiler/problems/P4003`).
- **Named and positional inputs** may not be mixed in one call
  (:doc:`/reference/compiler/problems/P4001`).
- **Each parameter** is named at most once in a call
  (:doc:`/reference/compiler/problems/P4024`).
- **An omitted input** keeps its value from the previous call, or its initial
  value on the first call: an instance keeps its variables between calls.
- **A negated output**, ``NOT over => ok``, stores the inverse of a ``BOOL``
  output (:doc:`/reference/compiler/problems/P4083` for any other type).

A call evaluates its inputs in the order the function block declares them,
whatever order the call writes them in, then runs the function block, then
stores its outputs in declaration order.

.. playground::

   FUNCTION_BLOCK Counter
     VAR_INPUT
       step : INT;
       reset : BOOL;
     END_VAR
     VAR_OUTPUT
       count : INT;
     END_VAR
     IF reset THEN
       count := 0;
     ELSE
       count := count + step;
     END_IF;
   END_FUNCTION_BLOCK

   PROGRAM main
     VAR
       counter : Counter;
       value : INT;
     END_VAR
     counter(step := 2, reset := FALSE);  (* named *)
     counter(3, FALSE);                   (* positional, in declaration order *)
     counter(reset := FALSE);             (* step keeps 3 from the previous call *)
     value := counter.count;
   END_PROGRAM

See Also
--------

- :doc:`function` — stateless callable unit
- :doc:`program` — top-level executable unit
- :doc:`/reference/standard-library/function-blocks/index` — standard function blocks
