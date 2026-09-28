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

.. code-block::

   FUNCTION Square : DINT
       VAR_INPUT
           x : DINT;
       END_VAR

       Square := x * x;
   END_FUNCTION

Functions may have ``VAR_INPUT`` and ``VAR_IN_OUT`` parameters and local
``VAR`` variables. Functions must not have ``VAR_OUTPUT`` parameters (use
function blocks for those). Because a function has no state, it cannot
declare or invoke a function block instance; that is reported as
:doc:`P4054 </reference/compiler/problems/P4054>`.

In/Out Parameters
-----------------

A ``VAR_IN_OUT`` parameter is passed by reference: the function reads and
writes the caller's variable, so a change inside the function is visible to
the caller after the call returns.

.. code-block::

   FUNCTION AddTo : DINT
       VAR_INPUT
           amount : DINT;
       END_VAR
       VAR_IN_OUT
           total : DINT;
       END_VAR
       total := total + amount;
       AddTo := total;
   END_FUNCTION

   PROGRAM main
       VAR
           sum : DINT := 100;
           result : DINT;
       END_VAR
       result := AddTo(amount := 42, total := sum);   (* sum is now 142 *)
   END_PROGRAM

The argument for a ``VAR_IN_OUT`` parameter must be a variable
(:doc:`P4058 </reference/compiler/problems/P4058>`) of exactly the
parameter's type (:doc:`P4059 </reference/compiler/problems/P4059>`) that
the compiler can prove is writable: not ``CONSTANT``, and not a ``VAR_INPUT``
of the calling POU (:doc:`P4060 </reference/compiler/problems/P4060>`).

Calling a Function
------------------

Functions can be called using positional or named (formal) arguments:

.. code-block::

   (* Positional *)
   result := Square(42);

   (* Named *)
   result := Square(x := 42);

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P4001` — Mixed named and positional arguments
- :doc:`/reference/compiler/problems/P4058` — ``VAR_IN_OUT`` argument is not a variable
- :doc:`/reference/compiler/problems/P4059` — ``VAR_IN_OUT`` argument type is not the parameter type
- :doc:`/reference/compiler/problems/P4060` — ``VAR_IN_OUT`` argument cannot be proved writable

See Also
--------

- :doc:`function-block` — stateful callable unit
- :doc:`program` — top-level executable unit
- :doc:`/reference/language/structured-text/function-call` — call syntax
