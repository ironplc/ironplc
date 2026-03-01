========
FUNCTION
========

A function is a stateless callable unit that returns a single value.
Functions do not retain state between calls.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1
   * - **Support**
     - Partial

Syntax
------

.. code-block:: bnf

   FUNCTION function_name : return_type
       variable_declarations
       statement_list
   END_FUNCTION

The function returns a value by assigning to the function name.

Example
-------

.. code-block:: iec61131

   FUNCTION Square : DINT
       VAR_INPUT
           x : DINT;
       END_VAR

       Square := x * x;
   END_FUNCTION

Functions may have ``VAR_INPUT`` parameters and local ``VAR`` variables.
Functions must not have ``VAR_OUTPUT`` or ``VAR_IN_OUT`` parameters
(use function blocks for those).

Calling a Function
------------------

Functions can be called using positional or named (formal) arguments:

.. code-block:: iec61131

   (* Positional *)
   result := Square(42);

   (* Named *)
   result := Square(x := 42);

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P4001` — Mixed named and positional arguments

See Also
--------

- :doc:`function-block` — stateful callable unit
- :doc:`program` — top-level executable unit
- :doc:`/reference/language/structured-text/function-call` — call syntax
