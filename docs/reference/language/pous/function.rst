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

Functions may have ``VAR_INPUT``, ``VAR_IN_OUT`` and ``VAR_OUTPUT``
parameters and local ``VAR`` variables. Because a function has no state, it
cannot declare or invoke a function block instance; that is reported as
:doc:`P4054 </reference/compiler/problems/P4054>`.

Output Parameters
-----------------

A ``VAR_OUTPUT`` parameter gives the caller a value besides the function's
result. A call stores it in a variable with ``=>`` after the function
returns:

.. code-block::

   FUNCTION DivMod : DINT
       VAR_INPUT
           a : DINT;
           b : DINT;
       END_VAR
       VAR_OUTPUT
           remainder : DINT;
       END_VAR

       DivMod := a / b;
       remainder := a MOD b;
   END_FUNCTION

   quotient := DivMod(a := 7, b := 2, remainder => rest);

An output starts from its initial value on every call, as a local variable
does. It is converted to the type of the variable it is stored in, as an
assigned value is, and one that variable cannot hold is
:doc:`P4035 </reference/compiler/problems/P4035>`. ``NOT done => idle``
stores the inverse of a ``BOOL`` output.

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

Positional arguments go to the ``VAR_INPUT`` and ``VAR_IN_OUT`` parameters in
the order the function declares them. A call supplies every one of them
(:doc:`P4018 </reference/compiler/problems/P4018>`), even one that declares an
initial value: a function keeps nothing between calls. Named arguments may be
written in any order, each parameter at most once
(:doc:`P4024 </reference/compiler/problems/P4024>`), and the call evaluates
them in the order the function declares its parameters.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P4001` — Mixed named and positional arguments
- :doc:`/reference/compiler/problems/P4004` — Output assignment names no output
- :doc:`/reference/compiler/problems/P4018` — Wrong number of arguments
- :doc:`/reference/compiler/problems/P4023` — Named argument is not a parameter
- :doc:`/reference/compiler/problems/P4024` — Parameter named twice
- :doc:`/reference/compiler/problems/P4035` — Output does not fit the variable it is stored in
- :doc:`/reference/compiler/problems/P4083` — Negated output is not a ``BOOL``
- :doc:`/reference/compiler/problems/P4058` — ``VAR_IN_OUT`` argument is not a variable
- :doc:`/reference/compiler/problems/P4059` — ``VAR_IN_OUT`` argument type is not the parameter type
- :doc:`/reference/compiler/problems/P4060` — ``VAR_IN_OUT`` argument cannot be proved writable

See Also
--------

- :doc:`function-block` — stateful callable unit
- :doc:`program` — top-level executable unit
- :doc:`/reference/language/structured-text/function-call` — call syntax
